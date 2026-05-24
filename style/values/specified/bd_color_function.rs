/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Side-channel typed payload for `color: -bd-spot(...)` /
//! `color: -bd-separation(...)` / `color: device-cmyk(...)` cascade values
//! (moegoe F2).
//!
//! The `color` longhand stores its computed value as
//! [`AbsoluteColor`](crate::color::AbsoluteColor) — a fixed RGBA quad with
//! no slot for a colorant name or a CMYK ink-coverage quad. An authored
//! `color: -bd-spot(PANTONE-185)` would therefore collapse to the
//! fail-closed `AbsoluteColor::TRANSPARENT_BLACK` returned by
//! `ColorFunction::<AbsoluteColor>::resolve_to_absolute` for `BdSpot`,
//! and a `color: device-cmyk(0 1 1 0)` would collapse to its naive sRGB
//! projection (CSS Color 4 §10.2.2) before any downstream consumer could
//! resolve it against a CMYK-aware backend.
//!
//! `BdColorFunction` is an internal-only longhand on the same
//! `inherited_text` style struct as `color`. The cascade hooks
//! [`PropertyDeclaration::Color`](crate::properties::PropertyDeclaration::Color)
//! application so that whenever `color` is set the matching
//! `_-bd-color-function` longhand is set alongside it — either
//! `BdColorFunction::Spot { ... }` when the specified colour is a
//! `ColorFunction::BdSpot`, `BdColorFunction::DeviceCmyk { ... }` when
//! the specified colour is a fallback-free `ColorFunction::DeviceCmyk`,
//! or `BdColorFunction::None` otherwise. The moegoe IR conversion
//! boundary reads this companion field and preserves either the
//! colorant name (Separation emission) or the CMYK quad (`k`/`K`
//! operator emission) through to PDF.
//!
//! The longhand is not author-exposed (`enabled_in = ""`); the only
//! supply path is the internal cascade hook.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::Atom;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// moegoe F2 — one resolved `(colorant-name, tint)` pair carried on the
/// internal `_-bd-color-function` companion for a
/// `color: device-n(...)` value (Stage C consumer).
///
/// Wrapping the pair in a named struct (rather than the
/// `(Atom, f32)` tuple it represents) is required because Stylo's
/// `ToAnimatedValue` blanket impl for `Vec<T>` requires `T:
/// ToAnimatedValue`, and Stylo provides no tuple impls for
/// `ToAnimatedValue`, `MallocSizeOf`, `ToShmem`, or
/// `SpecifiedValueInfo`. A named struct with the derives applied
/// is the supported path. Field names also make the cascade
/// synthesis and IR-conversion arms self-documenting.
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
pub struct BdDeviceNCompanionComponent {
    /// The colorant name (matches the colorant identifier authored
    /// on the `@-bd-colour` block that registers this DeviceN
    /// component).
    pub colorant: Atom,
    /// Resolved tint, clamped to `[0.0, 1.0]` per the PDF DeviceN
    /// tint-transform convention (ISO 32000-2 §8.6.6.5). Out-of-gamut
    /// values are preserved verbatim because the renderer clamps at
    /// emission time per backend convention.
    pub tint: f32,
}

/// Computed (and specified) value of the internal `_-bd-color-function`
/// longhand. `None` is the initial value; `Spot` carries the resolved
/// colorant name and tint produced by the cascade hook for
/// `color: -bd-spot(<name>[, <tint>])` /
/// `color: -bd-separation(<name>[, <tint>])`; `DeviceCmyk` carries the
/// resolved CMYK + alpha quad produced by the cascade hook for
/// `color: device-cmyk(c m y k[ / a])` when no fallback is authored.
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
pub enum BdColorFunction {
    /// No `-bd-spot()` / `-bd-separation()` / fallback-free
    /// `device-cmyk()` companion for the `color` longhand. This is the
    /// initial value and also the value the cascade hook stores when
    /// `color` is set to any colour without a fork-typed side channel.
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
    /// `color: device-cmyk(<c> <m> <y> <k>[ / <alpha>])` with NO
    /// authored fallback. Carried through the cascade as a typed quad
    /// so the moegoe IR boundary can preserve it onto
    /// `Color::DeviceCmyk` and emit the four-component `k` / `K` PDF
    /// operators (ISO 32000-2 §8.6.3 Table 75) rather than the naive
    /// sRGB projection (CSS Color 4 §10.2.2) that
    /// `ColorPropertyValue::to_computed_value` would otherwise produce.
    ///
    /// When the author supplies an explicit fallback
    /// (`device-cmyk(0 1 1 0, red)`) the companion remains `None`: CSS
    /// Color 6 §2 says the fallback drives the in-cascade colour space,
    /// so the `color` longhand's sRGB resolution is correct and the
    /// side channel must NOT override it.
    DeviceCmyk {
        /// Cyan ink coverage. Author-resolved scalar (numbers and
        /// percentages alike are normalised to a number-on-[0,1] range
        /// by `ColorComponent::resolve(None)`). Out-of-gamut values
        /// are preserved verbatim because the PDF `k` / `K` operator
        /// accepts the full real-number range; the renderer clamps at
        /// emission time per backend convention.
        c: f32,
        /// Magenta ink coverage.
        m: f32,
        /// Yellow ink coverage.
        y: f32,
        /// Key (black) ink coverage.
        k: f32,
        /// Alpha component. The CSS `device-cmyk()` `alpha-omitted`
        /// form (`device-cmyk(c m y k)`) yields `1.0` — matching CSS
        /// Color 4 §10.2.2.
        alpha: f32,
    },
    /// `color: device-n(<name> <tint>, … , <fallback>)` (or its
    /// `-bd-devicen(...)` alias). Carried through the cascade as a
    /// list of resolved (colorant-name, tint) pairs so the moegoe
    /// IR boundary can preserve it onto a DeviceN-aware IR colour
    /// node and emit the PDF DeviceN colour space (ISO 32000-2
    /// §8.6.6.5) rather than falling back to the authored sRGB
    /// fallback. Stage B carries the side-channel definition; Stage C
    /// (moegoe-side) wires the cascade synthesis and IR consumption.
    DeviceN {
        /// Resolved colorant tints. Each entry is the colorant name
        /// plus its 0..=1 ink-coverage tint (numbers and percentages
        /// alike normalise to a number-on-[0,1] range via
        /// `ColorComponent::resolve(None)`). Out-of-gamut values are
        /// preserved verbatim; the renderer clamps at emission time.
        pairs: Vec<BdDeviceNCompanionComponent>,
    },
}

impl BdColorFunction {
    /// Initial value (`None`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether this value carries a spot reference or a typed CMYK quad
    /// (i.e. anything other than [`Self::None`]).
    #[inline]
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl ToCss for BdColorFunction {
    /// Round-trip serialisation of the internal `_-bd-color-function`
    /// companion. The longhand is never web-exposed, so this is
    /// emitted only by debug tooling (computed-value dumps, devtools
    /// inspectors). The serialised form mirrors the authoring
    /// surface of the originating `color` value so a reader can
    /// reconstruct intent at a glance.
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        match self {
            Self::None => dest.write_str("none"),
            Self::Spot {
                name,
                tint,
                is_separation,
            } => {
                if *is_separation {
                    dest.write_str("-bd-separation(")?;
                } else {
                    dest.write_str("-bd-spot(")?;
                }
                // Atoms don't impl `ToCss` because they aren't a CSS
                // value type, but their string contents are a CSS
                // ident in this surface — emit them raw.
                dest.write_str(&name.to_string())?;
                if (tint - 1.0).abs() > f32::EPSILON {
                    dest.write_str(", ")?;
                    tint.to_css(dest)?;
                }
                dest.write_char(')')
            }
            Self::DeviceCmyk { c, m, y, k, alpha } => {
                dest.write_str("device-cmyk(")?;
                c.to_css(dest)?;
                dest.write_char(' ')?;
                m.to_css(dest)?;
                dest.write_char(' ')?;
                y.to_css(dest)?;
                dest.write_char(' ')?;
                k.to_css(dest)?;
                if (alpha - 1.0).abs() > f32::EPSILON {
                    dest.write_str(" / ")?;
                    alpha.to_css(dest)?;
                }
                dest.write_char(')')
            }
            Self::DeviceN { pairs } => {
                dest.write_str("device-n(")?;
                for (index, pair) in pairs.iter().enumerate() {
                    if index != 0 {
                        dest.write_str(", ")?;
                    }
                    // Atoms emit their string content as a raw CSS
                    // ident (matching `BdSpot` above). The companion
                    // longhand is internal-only, so the surface only
                    // ever reaches debug tooling.
                    dest.write_str(&pair.colorant.to_string())?;
                    dest.write_char(' ')?;
                    pair.tint.to_css(dest)?;
                }
                dest.write_char(')')
            }
        }
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
