/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Access to font metrics from the style system.

#![deny(missing_docs)]

use crate::values::computed::Length;

/// Represents the font metrics that style needs from a font to compute the
/// value of certain CSS units like `ex`.
#[derive(Clone, Debug, PartialEq)]
pub struct FontMetrics {
    /// The x-height of the font.
    pub x_height: Option<Length>,
    /// The zero advance. This is usually writing mode dependent
    pub zero_advance_measure: Option<Length>,
    /// The cap-height of the font.
    pub cap_height: Option<Length>,
    /// The ideographic-width of the font.
    pub ic_width: Option<Length>,
    /// The ascent of the font (a value is always available for this).
    pub ascent: Length,
    /// The descent of the font, as a positive distance below the baseline.
    pub descent: Option<Length>,
    /// The line gap (external leading) of the font.
    pub line_gap: Option<Length>,
    /// Script scale down factor for math-depth 1.
    /// https://w3c.github.io/mathml-core/#dfn-scriptpercentscaledown
    pub script_percent_scale_down: Option<f32>,
    /// Script scale down factor for math-depth 2.
    /// https://w3c.github.io/mathml-core/#dfn-scriptscriptpercentscaledown
    pub script_script_percent_scale_down: Option<f32>,
}

impl Default for FontMetrics {
    fn default() -> Self {
        FontMetrics {
            x_height: None,
            zero_advance_measure: None,
            cap_height: None,
            ic_width: None,
            ascent: Length::new(0.0),
            descent: None,
            line_gap: None,
            script_percent_scale_down: None,
            script_script_percent_scale_down: None,
        }
    }
}

impl FontMetrics {
    /// MathML Core's font-size scale between inherited and computed math depths.
    pub fn math_depth_scale(&self, inherited: i8, computed: i8) -> f32 {
        let (a, b) = (
            i32::from(inherited.min(computed)),
            i32::from(inherited.max(computed)),
        );
        if a == b {
            return 1.0;
        }
        let fallback = 0.71_f32;
        let first = self.script_percent_scale_down.unwrap_or(fallback);
        let second = self
            .script_script_percent_scale_down
            .unwrap_or(fallback * fallback);
        let mut exponent = b - a;
        let scale = if a <= 0 && b >= 2 {
            exponent -= 2;
            second
        } else if a == 1 {
            exponent -= 1;
            second / first
        } else if b == 1 {
            exponent -= 1;
            first
        } else {
            1.0
        } * fallback.powi(exponent);
        if computed < inherited {
            1.0 / scale.max(f32::MIN_POSITIVE)
        } else {
            scale
        }
    }

    /// Returns the x-height, computing a fallback value if not present
    pub fn x_height_or_default(&self, reference_font_size: Length) -> Length {
        // https://drafts.csswg.org/css-values/#ex
        //
        //     In the cases where it is impossible or impractical to
        //     determine the x-height, a value of 0.5em must be
        //     assumed.
        //
        // (But note we use 0.5em of the used, not computed
        // font-size)
        self.x_height.unwrap_or_else(|| reference_font_size * 0.5)
    }

    /// Returns the zero advance measure, computing a fallback value if not present
    pub fn zero_advance_measure_or_default(
        &self,
        reference_font_size: Length,
        upright: bool,
    ) -> Length {
        // https://drafts.csswg.org/css-values/#ch
        //
        //     In the cases where it is impossible or impractical to
        //     determine the measure of the “0” glyph, it must be
        //     assumed to be 0.5em wide by 1em tall. Thus, the ch
        //     unit falls back to 0.5em in the general case, and to
        //     1em when it would be typeset upright (i.e.
        //     writing-mode is vertical-rl or vertical-lr and
        //     text-orientation is upright).
        //
        // Same caveat about computed vs. used font-size applies
        // above.
        self.zero_advance_measure.unwrap_or_else(|| {
            if upright {
                reference_font_size
            } else {
                reference_font_size * 0.5
            }
        })
    }

    /// Returns the cap-height, computing a fallback value if not present
    pub fn cap_height_or_default(&self) -> Length {
        // https://drafts.csswg.org/css-values/#cap
        //
        //     In the cases where it is impossible or impractical to
        //     determine the cap-height, the font’s ascent must be
        //     used.
        //
        self.cap_height.unwrap_or_else(|| self.ascent)
    }

    /// Returns the font-derived `line-height: normal`, computing a fallback
    /// value if the font provides no descent metric
    pub fn normal_line_height_or_default(&self, reference_font_size: Length) -> Length {
        // https://drafts.csswg.org/css-inline-3/#valdef-line-height-normal
        //
        //     Set the used value to a "reasonable" value based on the
        //     font of the element.
        //
        // With metrics available the used value is the font's
        // ascent + descent + line gap; without a descent metric, fall
        // back to 1.2em (the CSS2 recommendation is a value between
        // 1.0 and 1.2 times the font size).
        match self.descent {
            Some(descent) => {
                let line_gap = self.line_gap.map_or(0., |gap| gap.px());
                Length::new(self.ascent.px() + descent.px().abs() + line_gap)
            },
            None => reference_font_size * 1.2,
        }
    }

    /// Returns the ideographic advance measure, computing a fallback value if not present
    pub fn ic_width_or_default(&self, reference_font_size: Length) -> Length {
        // https://drafts.csswg.org/css-values/#ic
        //
        //     In the cases where it is impossible or impractical to
        //     determine the ideographic advance measure, it must be
        //     assumed to be 1em.
        //
        // Same caveat about computed vs. used as for other
        // metric-dependent units.
        self.ic_width.unwrap_or_else(|| reference_font_size)
    }
}

/// Type of font metrics to retrieve.
#[derive(Clone, Debug, PartialEq)]
pub enum FontMetricsOrientation {
    /// Get metrics for horizontal or vertical according to the Context's
    /// writing mode, using horizontal metrics for vertical/mixed
    MatchContextPreferHorizontal,
    /// Get metrics for horizontal or vertical according to the Context's
    /// writing mode, using vertical metrics for vertical/mixed
    MatchContextPreferVertical,
    /// Force getting horizontal metrics.
    Horizontal,
}

#[cfg(test)]
mod tests {
    use super::FontMetrics;

    #[test]
    fn math_depth_uses_font_scales_and_reverses_depth_changes() {
        let metrics = FontMetrics {
            script_percent_scale_down: Some(0.8),
            script_script_percent_scale_down: Some(0.6),
            ..FontMetrics::default()
        };
        for (from, to, expected) in [
            (0, 0, 1.0),
            (0, 1, 0.8),
            (0, 2, 0.6),
            (1, 2, 0.75),
            (2, 3, 0.71),
            (-1, 2, 0.6 * 0.71),
        ] {
            assert!((metrics.math_depth_scale(from, to) - expected).abs() < 1e-6);
            assert!((metrics.math_depth_scale(to, from) - 1.0 / expected).abs() < 1e-6);
        }
        let fallback = FontMetrics::default();
        assert!((fallback.math_depth_scale(0, 2) - 0.71 * 0.71).abs() < 1e-6);
    }
}
