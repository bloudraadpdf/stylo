/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Side-channel typed payload for `color: -bd-spot(...)` / `-bd-separation(...)`
//! cascade values (moegoe F2).
//!
//! The `color` longhand stores its computed value as
//! [`AbsoluteColor`](crate::color::AbsoluteColor) — a fixed RGBA quad with
//! no slot for a colorant name. An authored `color: -bd-spot(PANTONE-185)`
//! would therefore collapse to the fail-closed
//! `AbsoluteColor::TRANSPARENT_BLACK` returned by
//! `ColorFunction::<AbsoluteColor>::resolve_to_absolute` for `BdSpot`,
//! losing the colorant name before any downstream consumer can resolve
//! it against the document `@-bd-colour` registry.
//!
//! `BdColorFunction` is an internal-only longhand on the same
//! `inherited_text` style struct as `color`. The cascade hooks
//! [`PropertyDeclaration::Color`](crate::properties::PropertyDeclaration::Color)
//! application so that whenever `color` is set the matching
//! `_-bd-color-function` longhand is set alongside it — either
//! `BdColorFunction::Spot { ... }` when the specified colour is a
//! `ColorFunction::BdSpot`, or `BdColorFunction::None` otherwise. The
//! moegoe IR conversion boundary reads this companion field and
//! preserves the colorant name through to PDF Separation emission.
//!
//! The longhand is not author-exposed (`enabled_in = ""`); the only
//! supply path is the internal cascade hook.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::Atom;
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// Computed (and specified) value of the internal `_-bd-color-function`
/// longhand. `None` is the initial value; `Spot` carries the resolved
/// colorant name and tint produced by the cascade hook for
/// `color: -bd-spot(<name>[, <tint>])` /
/// `color: -bd-separation(<name>[, <tint>])`.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdColorFunction {
    /// No `-bd-spot()` / `-bd-separation()` companion for the `color`
    /// longhand. This is the initial value and also the value the
    /// cascade hook stores when `color` is set to any non-spot colour.
    None,
    /// `color: -bd-spot(<name>[, <tint>])` (or `-bd-separation(...)`).
    /// `tint` is the 0..=1 resolved scalar (the cascade resolves
    /// percentages and numbers to a number-on-[0,1] range — matching
    /// the PDF Separation tint transform). `is_separation` records
    /// the authored spelling so the IR conversion can round-trip the
    /// `-bd-spot()` vs `-bd-separation()` choice.
    Spot {
        /// The colorant name (e.g. `PANTONE-185`) as a `crate::Atom`.
        name: Atom,
        /// Resolved tint, clamped to `[0.0, 1.0]`. `1.0` is the full
        /// ink coverage (the default when no tint is authored).
        tint: f32,
        /// `true` for `-bd-separation(...)`, `false` for `-bd-spot(...)`.
        /// Output PDF is identical; the flag is preserved for
        /// tooling and round-trip serialisation.
        is_separation: bool,
    },
}

impl BdColorFunction {
    /// Initial value (`None`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether this value carries a spot reference.
    #[inline]
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Spot { .. })
    }
}

impl Parse for BdColorFunction {
    /// `_-bd-color-function` is an internal companion of the `color`
    /// longhand. It is never parsed from author CSS — the cascade
    /// applies it as a side effect of applying `color: -bd-spot(...)`
    /// (see `cascade.rs::synthesise_bd_color_function_companion`). Any
    /// attempt to parse it returns a parse error so accidental
    /// exposure is loud at compile time.
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}
