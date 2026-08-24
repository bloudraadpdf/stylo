/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Output of parsing a color function, e.g. rgb(..), hsl(..), color(..)

use std::fmt::Write;

use super::{
    component::ColorComponent,
    convert::normalize_hue,
    parsing::{NumberOrAngleComponent, NumberOrPercentageComponent},
    AbsoluteColor, ColorFlags, ColorSpace,
};
use crate::derives::*;
use crate::values::{
    computed::color::Color as ComputedColor, generics::Optional, normalize,
    specified::color::Color as SpecifiedColor,
};
use cssparser::color::{clamp_floor_256_f32, OPAQUE};

/// moegoe F2 — one `(colorant-name, tint)` pair inside a
/// [`ColorFunction::BdDeviceN`] colour function.
///
/// Wrapping the pair in a named struct (rather than the
/// `(Atom, ColorComponent<...>)` tuple it represents) carries two
/// concrete benefits:
///
/// * `Vec<(A, B)>` does not implement `ToAnimatedValue` without a
///   blanket tuple impl, which Stylo does not provide. A named
///   struct with `#[derive(ToAnimatedValue)]` is the supported path
///   for compound payloads inside the `Vec<T>` blanket impl
///   ([`crate::values::animated::ToAnimatedValue for Vec<T>`]).
/// * Field names make the cascade and serialisation arms
///   self-documenting (`pair.colorant` vs `pair.0`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToShmem)]
#[repr(C)]
pub struct BdDeviceNComponent {
    /// The colorant name as a `crate::Atom` (matching the colorant
    /// identifier authored on the `@-bd-colour` block that registers
    /// this DeviceN component). Kept as `Atom` for the same reason as
    /// `BdSpot`: `Atom` has a `trivial_to_animated_value!` impl.
    pub colorant: crate::Atom,
    /// The authored tint component (number or percentage on `[0,1]`).
    /// Out-of-gamut values are preserved verbatim; the renderer
    /// clamps at emission time per backend convention.
    pub tint: ColorComponent<NumberOrPercentageComponent>,
}

/// Represents a specified color function.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToShmem)]
#[repr(u8)]
pub enum ColorFunction<OriginColor> {
    /// <https://drafts.csswg.org/css-color-4/#rgb-functions>
    Rgb(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // red
        ColorComponent<NumberOrPercentageComponent>, // green
        ColorComponent<NumberOrPercentageComponent>, // blue
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#the-hsl-notation>
    Hsl(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrAngleComponent>,      // hue
        ColorComponent<NumberOrPercentageComponent>, // saturation
        ColorComponent<NumberOrPercentageComponent>, // lightness
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#the-hwb-notation>
    Hwb(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrAngleComponent>,      // hue
        ColorComponent<NumberOrPercentageComponent>, // whiteness
        ColorComponent<NumberOrPercentageComponent>, // blackness
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#specifying-lab-lch>
    Lab(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // lightness
        ColorComponent<NumberOrPercentageComponent>, // a
        ColorComponent<NumberOrPercentageComponent>, // b
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#specifying-lab-lch>
    Lch(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // lightness
        ColorComponent<NumberOrPercentageComponent>, // chroma
        ColorComponent<NumberOrAngleComponent>,      // hue
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#specifying-oklab-oklch>
    Oklab(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // lightness
        ColorComponent<NumberOrPercentageComponent>, // a
        ColorComponent<NumberOrPercentageComponent>, // b
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#specifying-oklab-oklch>
    Oklch(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // lightness
        ColorComponent<NumberOrPercentageComponent>, // chroma
        ColorComponent<NumberOrAngleComponent>,      // hue
        ColorComponent<NumberOrPercentageComponent>, // alpha
    ),
    /// <https://drafts.csswg.org/css-color-4/#color-function>
    Color(
        Optional<OriginColor>,                       // origin
        ColorComponent<NumberOrPercentageComponent>, // red / x
        ColorComponent<NumberOrPercentageComponent>, // green / y
        ColorComponent<NumberOrPercentageComponent>, // blue / z
        ColorComponent<NumberOrPercentageComponent>, // alpha
        ColorSpace,
    ),
    /// A device-dependent CMYK colour with an optional fallback colour.
    DeviceCmyk(
        ColorComponent<NumberOrPercentageComponent>, // cyan
        ColorComponent<NumberOrPercentageComponent>, // magenta
        ColorComponent<NumberOrPercentageComponent>, // yellow
        ColorComponent<NumberOrPercentageComponent>, // key
        ColorComponent<NumberOrPercentageComponent>, // alpha
        Optional<Box<OriginColor>>,                  // fallback colour
    ),
    /// moegoe F2 — `-bd-spot(<name>[, <tint>])` / `-bd-separation(<name>[, <tint>])`.
    ///
    /// A named-spot-colour reference resolved against the document's
    /// `@-bd-colour` registry at the IR conversion boundary in
    /// `moegoe-css/src/computed_to_ir/`. The Stylo cascade carries the
    /// colorant name and authored tint opaquely; PDF emission renders
    /// the spot via the Separation colour space (ISO 32000-2 §8.6.6.4)
    /// or falls back to the alternate colour space when the registry
    /// has no matching entry.
    ///
    /// The `is_separation` flag distinguishes the two authoring spellings:
    /// `-bd-spot()` (PDFreactor / Prince ergonomic alias) carries
    /// `false`, `-bd-separation()` (PDF terminology) carries `true`.
    /// They produce identical PDF output today; the flag is kept so the
    /// IR conversion can preserve the authored spelling for tooling and
    /// serialisation round-trips.
    ///
    /// The colorant name is stored as a `crate::Atom` (rather than
    /// `AtomIdent`) so the value derives `ToAnimatedValue` trivially —
    /// `Atom` has a `trivial_to_animated_value!` impl in
    /// `values/animated/mod.rs`, while `GenericAtomIdent` does not.
    /// Round-tripping to/from `AtomIdent` happens at the parse and
    /// to-css boundaries.
    BdSpot(
        crate::Atom,                                 // colorant name
        ColorComponent<NumberOrPercentageComponent>, // tint
        bool,                                        // is_separation
    ),
    /// moegoe F2 — `device-n(<name> <tint>, … , <fallback>)` /
    /// `-bd-devicen(<name> <tint>, … , <fallback>)`.
    ///
    /// A multi-colorant DeviceN colour authored as N (colorant-name,
    /// tint-component) pairs plus a mandatory sRGB fallback colour.
    /// PDF emission renders this via the DeviceN colour space (ISO
    /// 32000-2 §8.6.6.5); the fallback is consumed when DeviceN is
    /// unavailable on the target backend or when the document is being
    /// projected to a colour space that has no compatible DeviceN
    /// resource (e.g. PDF/A-1 — see Stage A of the bladsy wire-through
    /// at bladsy commit `e93295e29c`).
    ///
    /// The colorant names are stored as `crate::Atom` for the same
    /// reason as [`Self::BdSpot`] — the variant must derive
    /// `ToAnimatedValue`, and `Atom` has the trivial impl.
    ///
    /// The fallback is stored as `Optional<Box<OriginColor>>` (not
    /// `Box<OriginColor>` directly) to keep [`Self::map_origin_color`]
    /// total: that helper's mapping closure returns `Option<U>`, and
    /// a fallback that becomes unmappable mid-pipeline must collapse
    /// gracefully rather than panicking. Author-supplied DeviceN
    /// always parses with `Some(...)`; CSS Color 5 §4 requires the
    /// fallback because no naïve projection exists for the
    /// open-ended set of DeviceN colorants. The `None` slot is
    /// therefore an internal pipeline state, never an authored value.
    BdDeviceN(
        Vec<BdDeviceNComponent>,    // (colorant, tint) pairs
        Optional<Box<OriginColor>>, // sRGB fallback (always Some at parse time)
    ),
}

impl ColorFunction<AbsoluteColor> {
    /// Try to resolve into a valid absolute color.
    pub fn resolve_to_absolute(&self) -> Result<AbsoluteColor, ()> {
        macro_rules! alpha {
            ($alpha:expr, $origin_color:expr) => {{
                $alpha
                    .resolve($origin_color)?
                    .map(|value| normalize(value.to_number(1.0)).clamp(0.0, OPAQUE))
            }};
        }

        Ok(match self {
            ColorFunction::Rgb(origin_color, r, g, b, alpha) => {
                // Use `color(srgb ...)` to serialize `rgb(...)` if an origin color is available;
                // missing components also require the modern syntax because
                // legacy rgb() cannot represent `none`.
                let use_color_syntax = origin_color.is_some()
                    || r.is_none()
                    || g.is_none()
                    || b.is_none()
                    || alpha.is_none();

                if use_color_syntax {
                    let origin_color = origin_color.as_ref().map(|origin| {
                        let origin = origin.to_color_space(ColorSpace::Srgb);
                        // Because rgb(..) syntax have components in range [0..255), we have to
                        // map them.
                        // NOTE: The IS_LEGACY_SRGB flag is not added back to the color, because
                        //       we're going to return the modern color(srgb ..) syntax.
                        AbsoluteColor::new(
                            ColorSpace::Srgb,
                            origin.c0().map(|v| v * 255.0),
                            origin.c1().map(|v| v * 255.0),
                            origin.c2().map(|v| v * 255.0),
                            origin.alpha(),
                        )
                    });

                    // We have to map all the components back to [0..1) range after all the
                    // calculations.
                    AbsoluteColor::new(
                        ColorSpace::Srgb,
                        r.resolve(origin_color.as_ref())?
                            .map(|c| c.to_number(255.0) / 255.0),
                        g.resolve(origin_color.as_ref())?
                            .map(|c| c.to_number(255.0) / 255.0),
                        b.resolve(origin_color.as_ref())?
                            .map(|c| c.to_number(255.0) / 255.0),
                        alpha!(alpha, origin_color.as_ref()),
                    )
                } else {
                    #[inline]
                    fn resolve(
                        component: &ColorComponent<NumberOrPercentageComponent>,
                        origin_color: Option<&AbsoluteColor>,
                    ) -> Result<u8, ()> {
                        Ok(clamp_floor_256_f32(
                            component
                                .resolve(origin_color)?
                                .map_or(0.0, |value| value.to_number(u8::MAX as f32)),
                        ))
                    }

                    let origin_color = origin_color.as_ref().map(|o| o.into_srgb_legacy());

                    AbsoluteColor::srgb_legacy(
                        resolve(r, origin_color.as_ref())?,
                        resolve(g, origin_color.as_ref())?,
                        resolve(b, origin_color.as_ref())?,
                        alpha!(alpha, origin_color.as_ref()).unwrap_or(0.0),
                    )
                }
            },
            ColorFunction::Hsl(origin_color, h, s, l, alpha) => {
                // Percent reference range for S and L: 0% = 0.0, 100% = 100.0
                const LIGHTNESS_RANGE: f32 = 100.0;
                const SATURATION_RANGE: f32 = 100.0;

                // If the origin color:
                // - was *NOT* specified, then we stick with the old way of serializing the
                //   value to rgb(..).
                // - was specified, we don't use the rgb(..) syntax, because we should allow the
                //   color to be out of gamut and not clamp.
                let use_rgb_sytax = origin_color.is_none()
                    && !h.is_none()
                    && !s.is_none()
                    && !l.is_none()
                    && !alpha.is_none();

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Hsl));

                let mut result = AbsoluteColor::new(
                    ColorSpace::Hsl,
                    h.resolve(origin_color.as_ref())?
                        .map(|angle| normalize_hue(angle.degrees())),
                    s.resolve(origin_color.as_ref())?.map(|s| {
                        if use_rgb_sytax {
                            s.to_number(SATURATION_RANGE).clamp(0.0, SATURATION_RANGE)
                        } else {
                            s.to_number(SATURATION_RANGE)
                        }
                    }),
                    l.resolve(origin_color.as_ref())?.map(|l| {
                        if use_rgb_sytax {
                            l.to_number(LIGHTNESS_RANGE).clamp(0.0, LIGHTNESS_RANGE)
                        } else {
                            l.to_number(LIGHTNESS_RANGE)
                        }
                    }),
                    alpha!(alpha, origin_color.as_ref()),
                );

                if use_rgb_sytax {
                    result.flags.insert(ColorFlags::IS_LEGACY_SRGB);
                }

                result
            },
            ColorFunction::Hwb(origin_color, h, w, b, alpha) => {
                // If the origin color:
                // - was *NOT* specified, then we stick with the old way of serializing the
                //   value to rgb(..).
                // - was specified, we don't use the rgb(..) syntax, because we should allow the
                //   color to be out of gamut and not clamp.
                let use_rgb_sytax = origin_color.is_none()
                    && !h.is_none()
                    && !w.is_none()
                    && !b.is_none()
                    && !alpha.is_none();

                // Percent reference range for W and B: 0% = 0.0, 100% = 100.0
                const WHITENESS_RANGE: f32 = 100.0;
                const BLACKNESS_RANGE: f32 = 100.0;

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Hwb));

                let mut result = AbsoluteColor::new(
                    ColorSpace::Hwb,
                    h.resolve(origin_color.as_ref())?
                        .map(|angle| normalize_hue(angle.degrees())),
                    w.resolve(origin_color.as_ref())?.map(|w| {
                        if use_rgb_sytax {
                            w.to_number(WHITENESS_RANGE).clamp(0.0, WHITENESS_RANGE)
                        } else {
                            w.to_number(WHITENESS_RANGE)
                        }
                    }),
                    b.resolve(origin_color.as_ref())?.map(|b| {
                        if use_rgb_sytax {
                            b.to_number(BLACKNESS_RANGE).clamp(0.0, BLACKNESS_RANGE)
                        } else {
                            b.to_number(BLACKNESS_RANGE)
                        }
                    }),
                    alpha!(alpha, origin_color.as_ref()),
                );

                if use_rgb_sytax {
                    result.flags.insert(ColorFlags::IS_LEGACY_SRGB);
                }

                result
            },
            ColorFunction::Lab(origin_color, l, a, b, alpha) => {
                // for L: 0% = 0.0, 100% = 100.0
                // for a and b: -100% = -125, 100% = 125
                const LIGHTNESS_RANGE: f32 = 100.0;
                const A_B_RANGE: f32 = 125.0;

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Lab));

                AbsoluteColor::new(
                    ColorSpace::Lab,
                    l.resolve(origin_color.as_ref())?
                        .map(|l| l.to_number(LIGHTNESS_RANGE)),
                    a.resolve(origin_color.as_ref())?
                        .map(|a| a.to_number(A_B_RANGE)),
                    b.resolve(origin_color.as_ref())?
                        .map(|b| b.to_number(A_B_RANGE)),
                    alpha!(alpha, origin_color.as_ref()),
                )
            },
            ColorFunction::Lch(origin_color, l, c, h, alpha) => {
                // for L: 0% = 0.0, 100% = 100.0
                // for C: 0% = 0, 100% = 150
                const LIGHTNESS_RANGE: f32 = 100.0;
                const CHROMA_RANGE: f32 = 150.0;

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Lch));

                AbsoluteColor::new(
                    ColorSpace::Lch,
                    l.resolve(origin_color.as_ref())?
                        .map(|l| l.to_number(LIGHTNESS_RANGE)),
                    c.resolve(origin_color.as_ref())?
                        .map(|c| c.to_number(CHROMA_RANGE)),
                    h.resolve(origin_color.as_ref())?
                        .map(|angle| normalize_hue(angle.degrees())),
                    alpha!(alpha, origin_color.as_ref()),
                )
            },
            ColorFunction::Oklab(origin_color, l, a, b, alpha) => {
                // for L: 0% = 0.0, 100% = 1.0
                // for a and b: -100% = -0.4, 100% = 0.4
                const LIGHTNESS_RANGE: f32 = 1.0;
                const A_B_RANGE: f32 = 0.4;

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Oklab));

                AbsoluteColor::new(
                    ColorSpace::Oklab,
                    l.resolve(origin_color.as_ref())?
                        .map(|l| l.to_number(LIGHTNESS_RANGE)),
                    a.resolve(origin_color.as_ref())?
                        .map(|a| a.to_number(A_B_RANGE)),
                    b.resolve(origin_color.as_ref())?
                        .map(|b| b.to_number(A_B_RANGE)),
                    alpha!(alpha, origin_color.as_ref()),
                )
            },
            ColorFunction::Oklch(origin_color, l, c, h, alpha) => {
                // for L: 0% = 0.0, 100% = 1.0
                // for C: 0% = 0.0 100% = 0.4
                const LIGHTNESS_RANGE: f32 = 1.0;
                const CHROMA_RANGE: f32 = 0.4;

                let origin_color = origin_color
                    .as_ref()
                    .map(|o| o.to_color_space(ColorSpace::Oklch));

                AbsoluteColor::new(
                    ColorSpace::Oklch,
                    l.resolve(origin_color.as_ref())?
                        .map(|l| l.to_number(LIGHTNESS_RANGE)),
                    c.resolve(origin_color.as_ref())?
                        .map(|c| c.to_number(CHROMA_RANGE)),
                    h.resolve(origin_color.as_ref())?
                        .map(|angle| normalize_hue(angle.degrees())),
                    alpha!(alpha, origin_color.as_ref()),
                )
            },
            ColorFunction::Color(origin_color, r, g, b, alpha, color_space) => {
                let origin_color = origin_color.as_ref().map(|o| {
                    let mut result = o.to_color_space(*color_space);

                    // If the origin color was a `rgb(..)` function, we should
                    // make sure it doesn't have the legacy flag any more so
                    // that it is recognized as a `color(srgb ..)` function.
                    result.flags.set(ColorFlags::IS_LEGACY_SRGB, false);

                    result
                });

                AbsoluteColor::new(
                    *color_space,
                    r.resolve(origin_color.as_ref())?.map(|c| c.to_number(1.0)),
                    g.resolve(origin_color.as_ref())?.map(|c| c.to_number(1.0)),
                    b.resolve(origin_color.as_ref())?.map(|c| c.to_number(1.0)),
                    alpha!(alpha, origin_color.as_ref()),
                )
            },
            ColorFunction::BdSpot(_name, _tint, _is_separation) => {
                // F2 named-spot colours cannot resolve to an `AbsoluteColor`
                // without the document `@-bd-colour` registry, which is not
                // in scope at this layer. moegoe's IR-conversion boundary
                // (`computed_to_ir/colour.rs`) intercepts the value before
                // any consumer asks for an absolute resolution and resolves
                // the colorant name to its registered alternate. Returning
                // `TRANSPARENT_BLACK` here is the fail-closed default —
                // it surfaces unresolved cascade leakage as transparent
                // pixels rather than a wrong colour.
                AbsoluteColor::TRANSPARENT_BLACK
            },
            ColorFunction::BdDeviceN(_pairs, fallback) => {
                // F2 DeviceN colours: when no DeviceN-aware backend is
                // resolving the value, fall back to the authored sRGB
                // fallback colour. CSS Color 5 §4 mandates that DeviceN
                // carry a usable fallback for exactly this case (unlike
                // `device-cmyk()`, which can derive a naïve sRGB
                // projection from its four components). The fallback is
                // already an `AbsoluteColor` at this `impl` slot.
                match fallback {
                    Optional::Some(fallback) => **fallback,
                    Optional::None => AbsoluteColor::TRANSPARENT_BLACK,
                }
            },
            ColorFunction::DeviceCmyk(c, m, y, k, alpha, fallback) => {
                if let Some(fallback) = fallback.as_ref() {
                    let mut fallback = **fallback;
                    if !matches!(alpha, ColorComponent::AlphaOmitted) {
                        fallback.alpha = alpha!(alpha, None).unwrap_or(fallback.alpha);
                    }
                    fallback
                } else {
                    let resolve_component =
                        |component: &ColorComponent<NumberOrPercentageComponent>| {
                            component
                                .resolve(None)
                                .map(|value| value.map(|value| value.to_number(1.0)))
                        };

                    let cyan = resolve_component(c)?.unwrap_or(0.0).clamp(0.0, 1.0);
                    let magenta = resolve_component(m)?.unwrap_or(0.0).clamp(0.0, 1.0);
                    let yellow = resolve_component(y)?.unwrap_or(0.0).clamp(0.0, 1.0);
                    let key = resolve_component(k)?.unwrap_or(0.0).clamp(0.0, 1.0);

                    let mut result = AbsoluteColor::new(
                        ColorSpace::Srgb,
                        (1.0 - cyan) * (1.0 - key),
                        (1.0 - magenta) * (1.0 - key),
                        (1.0 - yellow) * (1.0 - key),
                        alpha!(alpha, None),
                    );
                    result.flags = ColorFlags::IS_LEGACY_SRGB;
                    result
                }
            },
        })
    }
}

impl ColorFunction<SpecifiedColor> {
    /// Return true if the color funciton has an origin color specified.
    pub fn has_origin_color(&self) -> bool {
        match self {
            Self::Rgb(origin_color, ..)
            | Self::Hsl(origin_color, ..)
            | Self::Hwb(origin_color, ..)
            | Self::Lab(origin_color, ..)
            | Self::Lch(origin_color, ..)
            | Self::Oklab(origin_color, ..)
            | Self::Oklch(origin_color, ..)
            | Self::Color(origin_color, ..) => origin_color.is_some(),
            Self::DeviceCmyk(..) => false,
            Self::BdSpot(..) => false,
            Self::BdDeviceN(..) => false,
        }
    }

    /// Whether this function uses a modern colour syntax for interpolation.
    pub fn has_modern_syntax(&self) -> bool {
        match self {
            Self::Rgb(origin, red, green, blue, alpha) => {
                origin.is_some()
                    || red.is_none()
                    || green.is_none()
                    || blue.is_none()
                    || alpha.is_none()
            },
            Self::Hsl(origin, hue, saturation, lightness, alpha) => {
                origin.is_some()
                    || hue.is_none()
                    || saturation.is_none()
                    || lightness.is_none()
                    || alpha.is_none()
            },
            Self::Hwb(origin, hue, whiteness, blackness, alpha) => {
                origin.is_some()
                    || hue.is_none()
                    || whiteness.is_none()
                    || blackness.is_none()
                    || alpha.is_none()
            },
            Self::Lab(..)
            | Self::Lch(..)
            | Self::Oklab(..)
            | Self::Oklch(..)
            | Self::Color(..)
            | Self::DeviceCmyk(..)
            | Self::BdSpot(..)
            | Self::BdDeviceN(..) => true,
        }
    }

    /// Resolve element-dependent colour components at computed-value time.
    pub fn to_computed_value(&self, context: &crate::values::computed::Context) -> Self {
        macro_rules! compute {
            ($variant:ident, $origin:expr, $c0:expr, $c1:expr, $c2:expr, $alpha:expr) => {
                Self::$variant(
                    $origin.clone(),
                    $c0.to_computed_value(context),
                    $c1.to_computed_value(context),
                    $c2.to_computed_value(context),
                    $alpha.to_computed_value(context),
                )
            };
        }

        match self {
            Self::Rgb(origin, c0, c1, c2, alpha) => {
                compute!(Rgb, origin, c0, c1, c2, alpha)
            },
            Self::Hsl(origin, c0, c1, c2, alpha) => {
                compute!(Hsl, origin, c0, c1, c2, alpha)
            },
            Self::Hwb(origin, c0, c1, c2, alpha) => {
                compute!(Hwb, origin, c0, c1, c2, alpha)
            },
            Self::Lab(origin, c0, c1, c2, alpha) => {
                compute!(Lab, origin, c0, c1, c2, alpha)
            },
            Self::Lch(origin, c0, c1, c2, alpha) => {
                compute!(Lch, origin, c0, c1, c2, alpha)
            },
            Self::Oklab(origin, c0, c1, c2, alpha) => {
                compute!(Oklab, origin, c0, c1, c2, alpha)
            },
            Self::Oklch(origin, c0, c1, c2, alpha) => {
                compute!(Oklch, origin, c0, c1, c2, alpha)
            },
            Self::Color(origin, c0, c1, c2, alpha, color_space) => Self::Color(
                origin.clone(),
                c0.to_computed_value(context),
                c1.to_computed_value(context),
                c2.to_computed_value(context),
                alpha.to_computed_value(context),
                *color_space,
            ),
            Self::DeviceCmyk(c, m, y, k, alpha, fallback) => Self::DeviceCmyk(
                c.to_computed_value(context),
                m.to_computed_value(context),
                y.to_computed_value(context),
                k.to_computed_value(context),
                alpha.to_computed_value(context),
                fallback.clone(),
            ),
            Self::BdSpot(name, tint, is_separation) => Self::BdSpot(
                name.clone(),
                tint.to_computed_value(context),
                *is_separation,
            ),
            Self::BdDeviceN(pairs, fallback) => Self::BdDeviceN(
                pairs
                    .iter()
                    .map(|pair| BdDeviceNComponent {
                        colorant: pair.colorant.clone(),
                        tint: pair.tint.to_computed_value(context),
                    })
                    .collect(),
                fallback.clone(),
            ),
        }
    }

    /// Whether this function should remain a typed function at specified-value
    /// time rather than eagerly collapsing to an absolute colour.
    pub fn should_preserve_as_function(&self) -> bool {
        self.has_origin_color()
            || matches!(self, Self::DeviceCmyk(..))
            || matches!(self, Self::BdSpot(..))
            || matches!(self, Self::BdDeviceN(..))
    }

    /// Try to resolve the color function to an [`AbsoluteColor`] that does not
    /// contain any variables (currentcolor, color components, etc.).
    pub fn resolve_to_absolute(&self) -> Result<AbsoluteColor, ()> {
        match self {
            Self::DeviceCmyk(c, m, y, k, alpha, fallback) => {
                let fallback = match fallback.as_ref() {
                    Some(fallback) => Some(Box::new(fallback.resolve_to_absolute().ok_or(())?)),
                    None => None,
                };

                ColorFunction::DeviceCmyk(
                    c.clone(),
                    m.clone(),
                    y.clone(),
                    k.clone(),
                    alpha.clone(),
                    fallback.into(),
                )
                .resolve_to_absolute()
            },
            Self::BdSpot(name, tint, is_separation) => {
                // moegoe F2 — the spot reference cannot resolve to an
                // `AbsoluteColor` at this layer (the `@-bd-colour`
                // registry lives on `IrDocument`, not Stylo). Construct
                // the absolute-typed variant explicitly to disambiguate
                // `resolve_to_absolute` from its sibling impls.
                let absolute: ColorFunction<AbsoluteColor> =
                    ColorFunction::BdSpot(name.clone(), tint.clone(), *is_separation);
                absolute.resolve_to_absolute()
            },
            Self::BdDeviceN(pairs, fallback) => {
                // moegoe F2 — resolve the fallback colour eagerly so
                // the absolute-typed variant carries the projected sRGB
                // value the renderer should use whenever the DeviceN
                // colour space is unavailable. `Optional::as_ref()`
                // returns `std::option::Option`, mirroring the
                // `DeviceCmyk` arm above.
                let fallback = match fallback.as_ref() {
                    Some(fallback) => Some(Box::new(fallback.resolve_to_absolute().ok_or(())?)),
                    None => None,
                };
                let absolute: ColorFunction<AbsoluteColor> =
                    ColorFunction::BdDeviceN(pairs.clone(), fallback.into());
                absolute.resolve_to_absolute()
            },
            _ => {
                // Map the color function to one with an absolute origin color.
                let resolvable = self.map_origin_color(|o| o.resolve_to_absolute());
                resolvable.resolve_to_absolute()
            },
        }
    }
}

impl<Color> ColorFunction<Color> {
    /// Map colour dependencies to another type. Return None from `f` if the
    /// conversion fails.
    pub fn map_origin_color<U>(&self, mut f: impl FnMut(&Color) -> Option<U>) -> ColorFunction<U> {
        macro_rules! map {
            ($f:ident, $o:expr, $c0:expr, $c1:expr, $c2:expr, $alpha:expr) => {{
                ColorFunction::$f(
                    $o.as_ref().and_then(|value| f(value)).into(),
                    $c0.clone(),
                    $c1.clone(),
                    $c2.clone(),
                    $alpha.clone(),
                )
            }};
        }
        match self {
            ColorFunction::Rgb(o, c0, c1, c2, alpha) => map!(Rgb, o, c0, c1, c2, alpha),
            ColorFunction::Hsl(o, c0, c1, c2, alpha) => map!(Hsl, o, c0, c1, c2, alpha),
            ColorFunction::Hwb(o, c0, c1, c2, alpha) => map!(Hwb, o, c0, c1, c2, alpha),
            ColorFunction::Lab(o, c0, c1, c2, alpha) => map!(Lab, o, c0, c1, c2, alpha),
            ColorFunction::Lch(o, c0, c1, c2, alpha) => map!(Lch, o, c0, c1, c2, alpha),
            ColorFunction::Oklab(o, c0, c1, c2, alpha) => map!(Oklab, o, c0, c1, c2, alpha),
            ColorFunction::Oklch(o, c0, c1, c2, alpha) => map!(Oklch, o, c0, c1, c2, alpha),
            ColorFunction::Color(o, c0, c1, c2, alpha, color_space) => ColorFunction::Color(
                o.as_ref().and_then(|value| f(value)).into(),
                c0.clone(),
                c1.clone(),
                c2.clone(),
                alpha.clone(),
                color_space.clone(),
            ),
            ColorFunction::DeviceCmyk(c, m, y, k, alpha, fallback) => ColorFunction::DeviceCmyk(
                c.clone(),
                m.clone(),
                y.clone(),
                k.clone(),
                alpha.clone(),
                fallback
                    .as_ref()
                    .and_then(|value| f(value.as_ref()).map(Box::new))
                    .into(),
            ),
            ColorFunction::BdSpot(name, tint, is_separation) => {
                ColorFunction::BdSpot(name.clone(), tint.clone(), *is_separation)
            },
            ColorFunction::BdDeviceN(pairs, fallback) => ColorFunction::BdDeviceN(
                pairs.clone(),
                fallback
                    .as_ref()
                    .and_then(|value| f(value.as_ref()).map(Box::new))
                    .into(),
            ),
        }
    }
}

impl ColorFunction<ComputedColor> {
    /// Resolve a computed color function to an absolute computed color.
    pub fn resolve_to_absolute(&self, current_color: &AbsoluteColor) -> AbsoluteColor {
        // Map the color function to one with an absolute origin color.
        let resolvable = self.map_origin_color(|o| Some(o.resolve_to_absolute(current_color)));
        match resolvable.resolve_to_absolute() {
            Ok(color) => color,
            Err(..) => {
                debug_assert!(
                    false,
                    "the color could not be resolved even with a currentcolor specified?"
                );
                AbsoluteColor::TRANSPARENT_BLACK
            },
        }
    }
}

impl<C: style_traits::ToCss> style_traits::ToCss for ColorFunction<C> {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        if let Self::DeviceCmyk(c, m, y, k, alpha, fallback) = self {
            let is_opaque = if let ColorComponent::Value(value) = *alpha {
                value.to_number(OPAQUE) == OPAQUE
            } else {
                false
            };

            dest.write_str("device-cmyk(")?;
            c.to_css(dest)?;
            dest.write_str(" ")?;
            m.to_css(dest)?;
            dest.write_str(" ")?;
            y.to_css(dest)?;
            dest.write_str(" ")?;
            k.to_css(dest)?;
            if !is_opaque && !matches!(alpha, ColorComponent::AlphaOmitted) {
                dest.write_str(" / ")?;
                alpha.to_css(dest)?;
            }
            if let Optional::Some(fallback) = fallback {
                dest.write_str(", ")?;
                fallback.to_css(dest)?;
            }
            return dest.write_str(")");
        }

        if let Self::BdSpot(name, tint, is_separation) = self {
            // F2 — serialise the authored spelling so OM round-trips match
            // input. Tint defaults to 1.0 and is elided when authored as the
            // identity transform (matches PDFreactor / Prince `-ro-spot()` /
            // `-prince-color()` ergonomics).
            if *is_separation {
                dest.write_str("-bd-separation(")?;
            } else {
                dest.write_str("-bd-spot(")?;
            }
            crate::values::serialize_atom_identifier(name, dest)?;
            let tint_is_unity = match tint {
                ColorComponent::Value(v) => v.to_number(1.0) == 1.0,
                _ => false,
            };
            if !tint_is_unity {
                dest.write_str(", ")?;
                tint.to_css(dest)?;
            }
            return dest.write_str(")");
        }

        if let Self::BdDeviceN(pairs, fallback) = self {
            // F2 — serialise as `device-n(<name> <tint>, … , <fallback>)`.
            // The authored spelling is always preserved (the alias
            // `-bd-devicen(...)` is normalised to `device-n(...)` at
            // serialise time so OM round-trips of either form
            // produce a canonical output, matching the CSS Color 5
            // recommendation that vendor aliases serialise to the
            // standardised spelling).
            dest.write_str("device-n(")?;
            for (index, pair) in pairs.iter().enumerate() {
                if index != 0 {
                    dest.write_str(", ")?;
                }
                crate::values::serialize_atom_identifier(&pair.colorant, dest)?;
                dest.write_str(" ")?;
                pair.tint.to_css(dest)?;
            }
            if let Optional::Some(fallback) = fallback {
                if !pairs.is_empty() {
                    dest.write_str(", ")?;
                }
                fallback.to_css(dest)?;
            }
            return dest.write_str(")");
        }

        let (origin_color, alpha) = match self {
            Self::Rgb(origin_color, _, _, _, alpha) => {
                dest.write_str("rgb(")?;
                (origin_color, alpha)
            },
            Self::Hsl(origin_color, _, _, _, alpha) => {
                dest.write_str("hsl(")?;
                (origin_color, alpha)
            },
            Self::Hwb(origin_color, _, _, _, alpha) => {
                dest.write_str("hwb(")?;
                (origin_color, alpha)
            },
            Self::Lab(origin_color, _, _, _, alpha) => {
                dest.write_str("lab(")?;
                (origin_color, alpha)
            },
            Self::Lch(origin_color, _, _, _, alpha) => {
                dest.write_str("lch(")?;
                (origin_color, alpha)
            },
            Self::Oklab(origin_color, _, _, _, alpha) => {
                dest.write_str("oklab(")?;
                (origin_color, alpha)
            },
            Self::Oklch(origin_color, _, _, _, alpha) => {
                dest.write_str("oklch(")?;
                (origin_color, alpha)
            },
            Self::Color(origin_color, _, _, _, alpha, _) => {
                dest.write_str("color(")?;
                (origin_color, alpha)
            },
            Self::DeviceCmyk(..) => unreachable!("handled above"),
            Self::BdSpot(..) => unreachable!("handled above"),
            Self::BdDeviceN(..) => unreachable!("handled above"),
        };

        if let Optional::Some(origin_color) = origin_color {
            dest.write_str("from ")?;
            origin_color.to_css(dest)?;
            dest.write_str(" ")?;
        }

        let is_opaque = if let ColorComponent::Value(value) = *alpha {
            value.to_number(OPAQUE) == OPAQUE
        } else {
            false
        };

        macro_rules! serialize_alpha {
            ($alpha_component:expr) => {{
                if !is_opaque && !matches!($alpha_component, ColorComponent::AlphaOmitted) {
                    dest.write_str(" / ")?;
                    $alpha_component.to_css(dest)?;
                }
            }};
        }

        macro_rules! serialize_components {
            ($c0:expr, $c1:expr, $c2:expr) => {{
                debug_assert!(!matches!($c0, ColorComponent::AlphaOmitted));
                debug_assert!(!matches!($c1, ColorComponent::AlphaOmitted));
                debug_assert!(!matches!($c2, ColorComponent::AlphaOmitted));

                $c0.to_css(dest)?;
                dest.write_str(" ")?;
                $c1.to_css(dest)?;
                dest.write_str(" ")?;
                $c2.to_css(dest)?;
            }};
        }

        match self {
            Self::Rgb(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Hsl(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Hwb(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Lab(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Lch(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Oklab(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Oklch(_, c0, c1, c2, alpha) => {
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::Color(_, c0, c1, c2, alpha, color_space) => {
                color_space.to_css(dest)?;
                dest.write_str(" ")?;
                serialize_components!(c0, c1, c2);
                serialize_alpha!(alpha);
            },
            Self::DeviceCmyk(..) => unreachable!("handled above"),
            Self::BdSpot(..) => unreachable!("handled above"),
            Self::BdDeviceN(..) => unreachable!("handled above"),
        }

        dest.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: f32) -> ColorComponent<NumberOrPercentageComponent> {
        ColorComponent::Value(NumberOrPercentageComponent::Number(value))
    }

    #[test]
    fn rgb_missing_components_survive_absolute_resolution() {
        let function = ColorFunction::<AbsoluteColor>::Rgb(
            Optional::None,
            ColorComponent::None,
            number(255.0),
            ColorComponent::None,
            ColorComponent::AlphaOmitted,
        );
        let color = function
            .resolve_to_absolute()
            .expect("modern rgb() must resolve");

        assert_eq!(color.c0(), None);
        assert_eq!(color.c1(), Some(1.0));
        assert_eq!(color.c2(), None);
        assert!(!color.is_legacy_syntax());
    }

    #[test]
    fn hsl_missing_components_survive_absolute_resolution() {
        let function = ColorFunction::<AbsoluteColor>::Hsl(
            Optional::None,
            ColorComponent::Value(NumberOrAngleComponent::Angle(60.0)),
            ColorComponent::None,
            number(50.0),
            ColorComponent::AlphaOmitted,
        );
        let color = function
            .resolve_to_absolute()
            .expect("modern hsl() must resolve");

        assert_eq!(color.c0(), Some(60.0));
        assert_eq!(color.c1(), None);
        assert_eq!(color.c2(), Some(50.0));
    }
}
