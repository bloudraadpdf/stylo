/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::ColorFloat;
use super::{AbsoluteColor, ColorSpace};
use crate::values::generics::color::ColorLayerBlendMode;

/// Composite a source color over a backdrop with the selected blend mode.
pub fn composite(
    backdrop: &AbsoluteColor,
    source: &AbsoluteColor,
    mode: ColorLayerBlendMode,
) -> AbsoluteColor {
    let backdrop = backdrop.to_color_space(ColorSpace::Srgb);
    let source = source.to_color_space(ColorSpace::Srgb);
    let [br, bg, bb, ba] = *backdrop.raw_components();
    let [sr, sg, sb, sa] = *source.raw_components();
    let b = [br.clamp(0.0, 1.0), bg.clamp(0.0, 1.0), bb.clamp(0.0, 1.0)];
    let s = [sr.clamp(0.0, 1.0), sg.clamp(0.0, 1.0), sb.clamp(0.0, 1.0)];
    let blended = blend(b, s, mode);
    let alpha = sa + ba * (1.0 - sa);
    let mut result = [0.0; 3];
    if alpha > 0.0 {
        for i in 0..3 {
            result[i] =
                (sa * ((1.0 - ba) * s[i] + ba * blended[i]) + (1.0 - sa) * ba * b[i]) / alpha;
        }
    }
    AbsoluteColor::new(ColorSpace::Srgb, result[0], result[1], result[2], alpha)
}

fn blend(b: [ColorFloat; 3], s: [ColorFloat; 3], mode: ColorLayerBlendMode) -> [ColorFloat; 3] {
    use ColorLayerBlendMode::*;
    match mode {
        Hue => set_lum(set_sat(s, sat(b)), lum(b)),
        Saturation => set_lum(set_sat(b, sat(s)), lum(b)),
        Color => set_lum(s, lum(b)),
        Luminosity => set_lum(b, lum(s)),
        _ => {
            let mut result = [0.0; 3];
            for i in 0..3 {
                result[i] = blend_channel(b[i], s[i], mode).clamp(0.0, 1.0);
            }
            result
        },
    }
}

fn blend_channel(b: ColorFloat, s: ColorFloat, mode: ColorLayerBlendMode) -> ColorFloat {
    use ColorLayerBlendMode::*;
    match mode {
        Normal => s,
        Multiply => b * s,
        Screen => b + s - b * s,
        Overlay if b <= 0.5 => 2.0 * b * s,
        Overlay => (2.0 * b - 1.0) + s - (2.0 * b - 1.0) * s,
        Darken => b.min(s),
        Lighten => b.max(s),
        ColorDodge if b == 0.0 => 0.0,
        ColorDodge if s == 1.0 => 1.0,
        ColorDodge => (b / (1.0 - s)).min(1.0),
        ColorBurn if b == 1.0 => 1.0,
        ColorBurn if s == 0.0 => 0.0,
        ColorBurn => 1.0 - ((1.0 - b) / s).min(1.0),
        HardLight if s <= 0.5 => 2.0 * b * s,
        HardLight => b + (2.0 * s - 1.0) - b * (2.0 * s - 1.0),
        SoftLight if s <= 0.5 => b - (1.0 - 2.0 * s) * b * (1.0 - b),
        SoftLight => {
            let d = if b <= 0.25 {
                ((16.0 * b - 12.0) * b + 4.0) * b
            } else {
                b.sqrt()
            };
            b + (2.0 * s - 1.0) * (d - b)
        },
        Difference => (b - s).abs(),
        Exclusion => b + s - 2.0 * b * s,
        Hue | Saturation | Color | Luminosity => unreachable!(),
    }
}

fn lum(c: [ColorFloat; 3]) -> ColorFloat {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn sat(c: [ColorFloat; 3]) -> ColorFloat {
    c.iter()
        .copied()
        .fold(ColorFloat::NEG_INFINITY, ColorFloat::max)
        - c.iter()
            .copied()
            .fold(ColorFloat::INFINITY, ColorFloat::min)
}

fn clip_color(mut c: [ColorFloat; 3]) -> [ColorFloat; 3] {
    let l = lum(c);
    let n = c
        .iter()
        .copied()
        .fold(ColorFloat::INFINITY, ColorFloat::min);
    let x = c
        .iter()
        .copied()
        .fold(ColorFloat::NEG_INFINITY, ColorFloat::max);
    if n < 0.0 {
        for component in &mut c {
            *component = l + (*component - l) * l / (l - n);
        }
    }
    if x > 1.0 {
        for component in &mut c {
            *component = l + (*component - l) * (1.0 - l) / (x - l);
        }
    }
    c
}

fn set_lum(mut c: [ColorFloat; 3], l: ColorFloat) -> [ColorFloat; 3] {
    let difference = l - lum(c);
    for component in &mut c {
        *component += difference;
    }
    clip_color(c)
}

fn set_sat(mut c: [ColorFloat; 3], saturation: ColorFloat) -> [ColorFloat; 3] {
    let mut indices = [0, 1, 2];
    indices.sort_by(|&left, &right| c[left].total_cmp(&c[right]));
    let [min, mid, max] = indices;
    if c[max] > c[min] {
        c[mid] = (c[mid] - c[min]) * saturation / (c[max] - c[min]);
        c[max] = saturation;
    } else {
        c[mid] = 0.0;
        c[max] = 0.0;
    }
    c[min] = 0.0;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_over_uses_both_alpha_values() {
        let backdrop = AbsoluteColor::new(ColorSpace::Srgb, 0.0, 0.0, 1.0, 0.5);
        let source = AbsoluteColor::new(ColorSpace::Srgb, 1.0, 0.0, 0.0, 0.5);
        let result = composite(&backdrop, &source, ColorLayerBlendMode::Normal);
        let [r, g, b, a] = *result.raw_components();
        assert!((r - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(g, 0.0);
        assert!((b - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(a, 0.75);
    }

    #[test]
    fn blend_modes_use_unpremultiplied_channels() {
        assert_eq!(
            blend([0.5; 3], [0.5; 3], ColorLayerBlendMode::Multiply),
            [0.25; 3]
        );
        assert_eq!(
            blend([0.5; 3], [0.5; 3], ColorLayerBlendMode::Screen),
            [0.75; 3]
        );
        assert_eq!(
            blend([0.25; 3], [0.75; 3], ColorLayerBlendMode::Overlay),
            [0.375; 3]
        );
    }

    #[test]
    fn maps_input_layers_to_the_blending_gamut() {
        let backdrop = AbsoluteColor::new(ColorSpace::Srgb, 0.75, -0.1, 0.53, 1.0);
        let source = AbsoluteColor::new(ColorSpace::Srgb, 1.0, 1.0, 0.0, 0.5);
        let result = composite(&backdrop, &source, ColorLayerBlendMode::Normal);
        assert_eq!(result.components.1, 0.5);
    }
}
