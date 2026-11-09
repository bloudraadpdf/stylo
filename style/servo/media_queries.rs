/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Servo's media-query device and expression representation.

use crate::color::AbsoluteColor;
use crate::context::QuirksMode;
use crate::custom_properties::CssEnvironment;
use crate::derives::*;
use crate::font_metrics::FontMetrics;
use crate::logical_geometry::WritingMode;
use crate::media_queries::MediaType;
use crate::properties::style_structs::Font;
use crate::properties::ComputedValues;
use crate::queries::feature::{AllowsRanges, Evaluator, FeatureFlags, QueryFeatureDescription};
use crate::queries::values::{Orientation, PrefersColorScheme};
use crate::stylesheets::NavigationPhase;
use crate::values::computed::bd_gaps::{GapRuleList, GapRuleWidth};
use crate::values::computed::border::{BorderSideWidth, LineWidthKeywords};
use crate::values::computed::font::{FontFamilyList, GenericFontFamily};
use crate::values::computed::{
    CSSPixelLength, Context, Length, LineHeight, NonNegativeLength, Ratio, Resolution,
};
use crate::values::specified::color::{ColorSchemeFlags, ForcedColors};
use crate::values::specified::font::{
    FontSizeKeyword, QueryFontMetricsFlags, FONT_MEDIUM_CAP_PX, FONT_MEDIUM_CH_PX,
    FONT_MEDIUM_EX_PX, FONT_MEDIUM_IC_PX, FONT_MEDIUM_PX,
};
use crate::values::specified::ViewportVariant;
use crate::values::KeyframesName;
use app_units::{Au, AU_PER_PX};
use euclid::default::Size2D as UntypedSize2D;
use euclid::{Scale, SideOffsets2D, Size2D};
use mime::Mime;
use parking_lot::RwLock;
use servo_arc::Arc;
use std::fmt::Debug;
use std::mem;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use style_traits::{CSSPixel, DevicePixel};

/// A trait used to query font metrics in clients of Stylo. This is used by Device to
/// query font metrics in a way that is specific to the client using Stylo.
pub trait FontMetricsProvider: Debug + Sync {
    /// Query the font metrics for the given font and the given base font size.
    fn query_font_metrics(
        &self,
        vertical: bool,
        font: &Font,
        base_size: CSSPixelLength,
        flags: QueryFontMetricsFlags,
    ) -> FontMetrics;
    /// Gets the base size given a generic font family.
    fn base_size_for_generic(&self, generic: GenericFontFamily) -> Length;
    /// A client keyword size, or `None` to use the browser size table.
    fn font_size_keyword(&self, _keyword: FontSizeKeyword) -> Option<NonNegativeLength> {
        None
    }
    /// The generic family that governs font-size keyword resolution
    /// (`medium` and the rest of the CSS 2.1 ladder) for a family
    /// list. The default matches the browser quirk: only a list that
    /// IS a single bare generic resolves against that generic's base
    /// size. Clients may widen this -- print engines such as
    /// PDFreactor let a trailing `monospace` generic govern the
    /// keyword size even behind named families.
    fn keyword_size_generic_for_family(&self, family: &FontFamilyList) -> GenericFontFamily {
        family.single_generic().unwrap_or(GenericFontFamily::None)
    }
}

/// A user's preferred contrast level, independently of forced-color rendering.
#[derive(Clone, Copy, Debug, Default, Eq, FromPrimitive, Parse, PartialEq, ToCss)]
#[repr(u8)]
pub enum PreferenceContrast {
    /// No contrast preference was expressed.
    #[default]
    NoPreference,
    /// Prefer greater contrast.
    More,
    /// Prefer reduced contrast.
    Less,
    /// Use a custom contrast palette.
    Custom,
}

/// Effective user preferences for media features and supported color schemes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UserPreferences {
    /// An explicit origin-scoped color preference, independent of page scheme negotiation.
    pub color_scheme: Option<PrefersColorScheme>,
    /// Reduce non-essential motion.
    pub reduced_motion: bool,
    /// Reduce transparent and translucent effects.
    pub reduced_transparency: bool,
    /// Reduce data transfer.
    pub reduced_data: bool,
    /// The requested contrast level.
    pub contrast: PreferenceContrast,
    /// Use the device's forced-color palette.
    pub forced_colors: bool,
    /// The output colors are inverted.
    pub inverted_colors: bool,
}

/// A device is a structure that represents the current media a given document
/// is displayed in.
///
/// This is the struct against which media queries are evaluated.
/// and contains all the viewport rule state.
///
/// This structure also contains atomics used for computing root font-relative
/// units. These atomics use relaxed ordering, since when computing the style
/// of the root element, there can't be any other style being computed at the
/// same time (given we need the style of the parent to compute everything else).
#[derive(Debug, MallocSizeOf)]
pub struct Device {
    /// Document-lifetime random state, retained when the embedder replaces a style device.
    #[ignore_malloc_size_of = "Shared document random state"]
    random_state: std::sync::Arc<crate::values::random::DocumentRandomState>,
    /// The current media type used by de device.
    media_type: MediaType,
    /// The current viewport size, in CSS pixels.
    viewport_size: Size2D<f32, CSSPixel>,
    device_size: Size2D<f32, CSSPixel>,
    /// The current bleed-box size, in CSS pixels.
    ///
    /// Mirrors `viewport_size` but is consulted by the
    /// `PageRelativeLength` bleed-axis arms (`Bw`/`Bh`/`Bi`/`Bb`/
    /// `Bmin`/`Bmax`) rather than the viewport-relative path.
    /// Initialised to the same dimensions as `viewport_size`, so
    /// embedders that never call `set_bleed_box_size` get the
    /// page-axis fallback behaviour for free.
    bleed_box_size: Size2D<f32, CSSPixel>,
    /// The current page-box size, in CSS pixels.
    ///
    /// Mirrors `viewport_size` but is consulted by the `width` and
    /// `height` media features (`eval_width`/`eval_height`) rather
    /// than the viewport-relative path. In paged output the
    /// `width`/`height` media features describe the page box — the
    /// full sheet, page area plus margins (Media Queries 4 §6.1) —
    /// whereas viewport units (`vw`/`vh`/…) resolve against the page
    /// area, i.e. the content box inside the margins (CSS Values 4
    /// §6.1.2). The two therefore differ in paged media and cannot
    /// share `viewport_size`. Initialised to the same dimensions as
    /// `viewport_size`, so embedders that never call
    /// `set_page_box_size` get the viewport-equals-page-box fallback
    /// for free — correct for continuous media, where the two
    /// coincide.
    page_box_size: Size2D<f32, CSSPixel>,
    /// The page box that the page-relative length units
    /// (`-bd-p{w,h,i,b,min,max}`) resolve against, in CSS pixels: the
    /// first page with its margins. Media queries keep `page_box_size`,
    /// the paper size that no `@page` rule chooses (css-page-3 §7.1).
    /// Initialised to the same dimensions as `viewport_size`.
    page_unit_box_size: Size2D<f32, CSSPixel>,
    /// The current device pixel ratio, from CSS pixels to device pixels.
    device_pixel_ratio: Scale<f32, CSSPixel, DevicePixel>,
    /// The current quirks mode.
    #[ignore_malloc_size_of = "Pure stack type"]
    quirks_mode: QuirksMode,

    /// Current computed style of the root element, used for calculations of
    /// root font-relative units.
    #[ignore_malloc_size_of = "Arc"]
    root_style: RwLock<Arc<ComputedValues>>,
    /// Font size of the root element, used for rem units in other elements.
    #[ignore_malloc_size_of = "Pure stack type"]
    root_font_size: AtomicU32,
    /// X-height of the root element, used for rex units in other elements.
    #[ignore_malloc_size_of = "Pure stack type"]
    root_font_metrics_ex: AtomicU32,
    /// Cap-height of the root element, used for rcap units in other elements.
    #[ignore_malloc_size_of = "Pure stack type"]
    root_font_metrics_cap: AtomicU32,
    /// Advance measure (ch) of the root element, used for rch units in other elements.
    #[ignore_malloc_size_of = "Pure stack type"]
    root_font_metrics_ch: AtomicU32,
    /// Ideographic advance measure of the root element, used for ric units in other elements.
    #[ignore_malloc_size_of = "Pure stack type"]
    root_font_metrics_ic: AtomicU32,
    /// Whether any styles computed in the document relied on the root font-size
    /// by using rem units.
    #[ignore_malloc_size_of = "Pure stack type"]
    used_root_font_size: AtomicBool,
    /// Whether any styles computed in the document relied on the root line-height
    /// by using rlh units.
    #[ignore_malloc_size_of = "Pure stack type"]
    used_root_line_height: AtomicBool,
    /// Whether any styles computed in the document relied on the root font metrics
    /// by using rcap, rch, rex, or ric units. This is a lock instead of an atomic
    /// in order to prevent concurrent writes to the root font metric values.
    #[ignore_malloc_size_of = "Pure stack type"]
    used_root_font_metrics: RwLock<bool>,
    /// Whether any styles computed in the document relied on font metrics.
    used_font_metrics: AtomicBool,
    /// Whether any styles computed in the document relied on the viewport size.
    #[ignore_malloc_size_of = "Pure stack type"]
    used_viewport_units: AtomicBool,
    /// Whether the user prefers light mode or dark mode
    #[ignore_malloc_size_of = "Pure stack type"]
    prefers_color_scheme: PrefersColorScheme,
    #[ignore_malloc_size_of = "Pure stack type"]
    user_preferences: UserPreferences,
    /// The page's supported schemes, independent of the user or embedding preference.
    #[ignore_malloc_size_of = "Pure stack type"]
    page_color_scheme_support: ColorSchemeFlags,
    /// The navigation phase of the document, which `@navigation` rules query.
    #[ignore_malloc_size_of = "Pure stack type"]
    navigation_phase: NavigationPhase,
    /// The CssEnvironment object responsible of getting CSS environment
    /// variables.
    environment: CssEnvironment,
    /// An implementation of a trait which implements support for querying font metrics.
    #[ignore_malloc_size_of = "Owned by embedder"]
    font_metrics_provider: Box<dyn FontMetricsProvider>,
    /// The default computed values for this Device.
    #[ignore_malloc_size_of = "Arc is shared"]
    default_computed_values: Arc<ComputedValues>,
    /// The lengths of the line width keywords.
    line_width_keywords: LineWidthKeywords,
    /// Whether MathML uses Prince's display and minimum font-size semantics.
    pub prince_math_compat: bool,
    /// Whether background image layers use their authored blend modes.
    pub background_blending_enabled: bool,
    /// Whether substituted values support CSS comparison functions.
    pub comparison_functions_enabled: bool,
    /// Whether substituted grid-template values support subgrid tracks.
    pub subgrid_enabled: bool,
    /// Whether substituted automatic grid repetitions support non-fixed tracks.
    pub intrinsic_grid_repetition_enabled: bool,
    /// Whether direct link-state selectors win otherwise equal cascade ties.
    pub link_state_cascade_priority: bool,
    /// Whether a link-state selector can target its link's ::after pseudo-element.
    pub link_after_rules_enabled: bool,
}

impl Device {
    /// Trivially construct a new `Device`.
    pub fn new(
        media_type: MediaType,
        quirks_mode: QuirksMode,
        viewport_size: Size2D<f32, CSSPixel>,
        device_pixel_ratio: Scale<f32, CSSPixel, DevicePixel>,
        font_metrics_provider: Box<dyn FontMetricsProvider>,
        default_computed_values: Arc<ComputedValues>,
        prefers_color_scheme: PrefersColorScheme,
    ) -> Device {
        let default_values =
            ComputedValues::initial_values_with_font_override(Font::initial_values());
        let root_style = RwLock::new(Arc::clone(&default_values));
        Device {
            random_state: std::sync::Arc::new(crate::values::random::DocumentRandomState::default()),
            media_type,
            viewport_size,
            device_size: viewport_size,
            bleed_box_size: viewport_size,
            page_box_size: viewport_size,
            page_unit_box_size: viewport_size,
            device_pixel_ratio,
            quirks_mode,
            root_style,
            root_font_size: AtomicU32::new(FONT_MEDIUM_PX.to_bits()),
            root_font_metrics_ex: AtomicU32::new(FONT_MEDIUM_EX_PX.to_bits()),
            root_font_metrics_cap: AtomicU32::new(FONT_MEDIUM_CAP_PX.to_bits()),
            root_font_metrics_ch: AtomicU32::new(FONT_MEDIUM_CH_PX.to_bits()),
            root_font_metrics_ic: AtomicU32::new(FONT_MEDIUM_IC_PX.to_bits()),
            used_root_font_size: AtomicBool::new(false),
            used_root_line_height: AtomicBool::new(false),
            used_root_font_metrics: RwLock::new(false),
            used_font_metrics: AtomicBool::new(false),
            used_viewport_units: AtomicBool::new(false),
            prefers_color_scheme,
            user_preferences: UserPreferences::default(),
            page_color_scheme_support: ColorSchemeFlags::empty(),
            navigation_phase: NavigationPhase::Idle,
            environment: CssEnvironment::default(),
            font_metrics_provider,
            default_computed_values,
            line_width_keywords: LineWidthKeywords::css(),
            prince_math_compat: false,
            background_blending_enabled: true,
            comparison_functions_enabled: true,
            subgrid_enabled: true,
            intrinsic_grid_repetition_enabled: true,
            link_state_cascade_priority: false,
            link_after_rules_enabled: true,
        }
    }

    /// The random base state belonging to this device's document.
    pub fn random_state(&self) -> &std::sync::Arc<crate::values::random::DocumentRandomState> {
        &self.random_state
    }

    /// Preserve the document's random state across replacement devices.
    pub fn set_random_state(&mut self, state: std::sync::Arc<crate::values::random::DocumentRandomState>) {
        self.random_state = state;
    }

    /// The lengths of the `thin`, `medium` and `thick` line widths.
    #[inline]
    pub fn line_width_keywords(&self) -> LineWidthKeywords {
        self.line_width_keywords
    }

    /// Sets the lengths of the line width keywords, and `medium` as the initial width of borders, outlines and gap
    /// rules.
    pub fn set_line_width_keywords(&mut self, keywords: LineWidthKeywords) {
        let medium = BorderSideWidth(keywords.medium);
        let gap_rule = GapRuleList::single(GapRuleWidth::new(keywords.medium, Au::from_px(1)));
        let mut values = (*self.default_computed_values).clone();
        let border = values.mutate_border();
        border.set_border_top_width(medium.clone());
        border.set_border_right_width(medium.clone());
        border.set_border_bottom_width(medium.clone());
        border.set_border_left_width(medium.clone());
        values.mutate_outline().set_outline_width(medium);
        let column = values.mutate_column();
        column.set_column_rule_width(gap_rule.clone());
        column.set_row_rule_width(gap_rule);
        self.default_computed_values = Arc::new(values);
        self.line_width_keywords = keywords;
    }

    /// Get the relevant environment to resolve `env()` functions.
    #[inline]
    pub fn environment(&self) -> &CssEnvironment {
        &self.environment
    }

    /// Return the default computed values for this device.
    pub fn default_computed_values(&self) -> &ComputedValues {
        &self.default_computed_values
    }

    /// Store a pointer to the root element's computed style, for use in
    /// calculation of root font-relative metrics.
    pub fn set_root_style(&self, style: &Arc<ComputedValues>) {
        *self.root_style.write() = style.clone();
    }

    /// Get the font size of the root element (for rem)
    pub fn root_font_size(&self) -> CSSPixelLength {
        self.used_root_font_size.store(true, Ordering::Relaxed);
        CSSPixelLength::new(f32::from_bits(self.root_font_size.load(Ordering::Relaxed)))
    }

    /// Set the font size of the root element (for rem), in zoom-independent CSS pixels.
    pub fn set_root_font_size(&self, size: f32) {
        self.root_font_size.store(size.to_bits(), Ordering::Relaxed)
    }

    /// Get the line height of the root element (for rlh), in zoom-independent CSS pixels.
    ///
    /// It derives from the root style on use, so that a document without rlh units
    /// queries no font metrics for it.
    pub fn root_line_height(&self) -> CSSPixelLength {
        self.used_root_line_height.store(true, Ordering::Relaxed);
        let root_style = self.root_style.read();
        let line_height =
            self.calc_line_height(&root_style.get_font(), root_style.writing_mode, None);
        CSSPixelLength::new(root_style.effective_zoom.unzoom(line_height.0.px()))
    }

    /// Get the x-height of the root element (for rex)
    pub fn root_font_metrics_ex(&self) -> Length {
        self.ensure_root_font_metrics_updated();
        Length::new(f32::from_bits(
            self.root_font_metrics_ex.load(Ordering::Relaxed),
        ))
    }

    /// Set the x-height of the root element (for rex), in zoom-independent CSS pixels.
    pub fn set_root_font_metrics_ex(&self, size: f32) -> bool {
        let size = size.to_bits();
        let previous = self.root_font_metrics_ex.swap(size, Ordering::Relaxed);
        previous != size
    }

    /// Get the cap-height of the root element (for rcap)
    pub fn root_font_metrics_cap(&self) -> Length {
        self.ensure_root_font_metrics_updated();
        Length::new(f32::from_bits(
            self.root_font_metrics_cap.load(Ordering::Relaxed),
        ))
    }

    /// Set the cap-height of the root element (for rcap), in zoom-independent CSS pixels.
    pub fn set_root_font_metrics_cap(&self, size: f32) -> bool {
        let size = size.to_bits();
        let previous = self.root_font_metrics_cap.swap(size, Ordering::Relaxed);
        previous != size
    }

    /// Get the advance measure of the root element (for rch)
    pub fn root_font_metrics_ch(&self) -> Length {
        self.ensure_root_font_metrics_updated();
        Length::new(f32::from_bits(
            self.root_font_metrics_ch.load(Ordering::Relaxed),
        ))
    }

    /// Set the advance measure of the root element (for rch), in zoom-independent CSS pixels.
    pub fn set_root_font_metrics_ch(&self, size: f32) -> bool {
        let size = size.to_bits();
        let previous = self.root_font_metrics_ch.swap(size, Ordering::Relaxed);
        previous != size
    }

    /// Get the ideographic advance measure of the root element (for ric)
    pub fn root_font_metrics_ic(&self) -> Length {
        self.ensure_root_font_metrics_updated();
        Length::new(f32::from_bits(
            self.root_font_metrics_ic.load(Ordering::Relaxed),
        ))
    }

    /// Set the ideographic advance measure of the root element (for ric), in zoom-independent CSS pixels.
    pub fn set_root_font_metrics_ic(&self, size: f32) -> bool {
        let size = size.to_bits();
        let previous = self.root_font_metrics_ic.swap(size, Ordering::Relaxed);
        previous != size
    }

    /// Returns the computed line-height for the font in a given computed values instance.
    ///
    /// If you pass down an element, then the used line-height is returned.
    pub fn calc_line_height(
        &self,
        font: &crate::properties::style_structs::Font,
        writing_mode: WritingMode,
        _element: Option<()>,
    ) -> NonNegativeLength {
        (match font.line_height {
            LineHeight::Normal | LineHeight::None => {
                // `normal` derives from the first available font's
                // metrics (ascent + descent + line gap), per
                // css-inline-3, with a 1.2em fallback when the font
                // provides no descent metric.
                let font_size = font.font_size.computed_size();
                let metrics = self.query_font_metrics(
                    writing_mode.is_text_vertical(),
                    font,
                    font_size,
                    QueryFontMetricsFlags::USE_USER_FONT_SET,
                    /* track_usage = */ true,
                );
                metrics.normal_line_height_or_default(font_size)
            },
            LineHeight::Number(number) => font.font_size.computed_size() * number.0,
            LineHeight::Length(length) => length.0,
        })
        .into()
    }

    /// Get the quirks mode of the current device.
    pub fn quirks_mode(&self) -> QuirksMode {
        self.quirks_mode
    }

    /// Sets the body text color for the "inherit color from body" quirk.
    ///
    /// <https://quirks.spec.whatwg.org/#the-tables-inherit-color-from-body-quirk>
    pub fn set_body_text_color(&self, _color: AbsoluteColor) {
        // Servo doesn't implement this quirk (yet)
    }

    /// Gets the base size given a generic font family.
    pub fn base_size_for_generic(&self, generic: GenericFontFamily) -> Length {
        self.font_metrics_provider.base_size_for_generic(generic)
    }

    /// A client keyword size, or `None` to use the browser size table.
    pub fn font_size_keyword(&self, keyword: FontSizeKeyword) -> Option<NonNegativeLength> {
        self.font_metrics_provider.font_size_keyword(keyword)
    }

    /// The generic family that governs font-size keyword resolution for
    /// `family` -- see [`FontMetricsProvider::keyword_size_generic_for_family`].
    pub fn keyword_size_generic_for_family(&self, family: &FontFamilyList) -> GenericFontFamily {
        self.font_metrics_provider
            .keyword_size_generic_for_family(family)
    }

    /// Whether a given animation name may be referenced from style.
    pub fn animation_name_may_be_referenced(&self, _: &KeyframesName) -> bool {
        // Assume it is, since we don't have any good way to prove it's not.
        true
    }

    /// Returns whether we ever looked up the root font size of the Device.
    pub fn used_root_font_size(&self) -> bool {
        self.used_root_font_size.load(Ordering::Relaxed)
    }

    /// Returns whether we ever looked up the root line-height of the device.
    pub fn used_root_line_height(&self) -> bool {
        self.used_root_line_height.load(Ordering::Relaxed)
    }

    /// Returns whether we ever looked up the root font metrics of the device.
    pub fn used_root_font_metrics(&self) -> bool {
        *self.used_root_font_metrics.read()
    }

    /// Returns whether font metrics have been queried.
    pub fn used_font_metrics(&self) -> bool {
        self.used_font_metrics.load(Ordering::Relaxed)
    }

    /// Get the viewport size on this [`Device`].
    pub fn viewport_size(&self) -> Size2D<f32, CSSPixel> {
        self.viewport_size
    }

    /// Set the viewport size on this [`Device`].
    ///
    /// Note that this does not update any associated `Stylist`. For this you must call
    /// `Stylist::media_features_change_changed_style` and
    /// `Stylist::force_stylesheet_origins_dirty`.
    pub fn set_viewport_size(&mut self, viewport_size: Size2D<f32, CSSPixel>) {
        self.viewport_size = viewport_size;
    }

    /// Get the bleed-box size on this [`Device`].
    pub fn bleed_box_size(&self) -> Size2D<f32, CSSPixel> {
        self.bleed_box_size
    }

    /// Set the bleed-box size on this [`Device`].
    ///
    /// moegoe calls this from
    /// `StyleEngine::update_bleed_box_for_page` after computing the
    /// cascaded `@page { bleed: ... }` outset of the cascaded page
    /// `size`. Like `set_viewport_size`, this does not update any
    /// associated `Stylist`; the caller mirrors the
    /// `media_features_change_changed_style` /
    /// `force_stylesheet_origins_dirty` calls so the next compute
    /// pass re-evaluates `-bd-b*` lengths.
    pub fn set_bleed_box_size(&mut self, bleed_box_size: Size2D<f32, CSSPixel>) {
        self.bleed_box_size = bleed_box_size;
    }

    /// Get the page-box size on this [`Device`].
    pub fn page_box_size(&self) -> Size2D<f32, CSSPixel> {
        self.page_box_size
    }

    /// Set the page-box size on this [`Device`].
    ///
    /// moegoe calls this from
    /// `StyleEngine::update_page_box_for_media_queries` with the
    /// cascaded `@page` size — the full sheet (page area plus
    /// margins) — so the `width`/`height` media features describe the
    /// page box per Media Queries 4 §6.1, while `viewport_size`
    /// keeps the page area for viewport-unit resolution. Like
    /// `set_viewport_size`, this does not update any associated
    /// `Stylist`; the caller mirrors the
    /// `media_features_change_changed_style` /
    /// `force_stylesheet_origins_dirty` calls so the next compute
    /// pass re-evaluates `width`/`height` media queries.
    pub fn set_page_box_size(&mut self, page_box_size: Size2D<f32, CSSPixel>) {
        self.page_box_size = page_box_size;
    }

    /// Get the page box of the page-relative length units on this [`Device`].
    pub fn page_unit_box_size(&self) -> Size2D<f32, CSSPixel> {
        self.page_unit_box_size
    }

    /// Set the page box of the page-relative length units on this
    /// [`Device`]. Like `set_viewport_size`, this does not update any
    /// associated `Stylist`.
    pub fn set_page_unit_box_size(&mut self, page_unit_box_size: Size2D<f32, CSSPixel>) {
        self.page_unit_box_size = page_unit_box_size;
    }

    /// Returns the viewport size of the current device in app units, needed,
    /// among other things, to resolve viewport units.
    #[inline]
    pub fn au_viewport_size(&self) -> UntypedSize2D<Au> {
        Size2D::new(
            Au::from_f32_px(self.viewport_size.width),
            Au::from_f32_px(self.viewport_size.height),
        )
    }

    /// Like the above, but records that we've used viewport units.
    pub fn au_viewport_size_for_viewport_unit_resolution(
        &self,
        _: ViewportVariant,
    ) -> UntypedSize2D<Au> {
        self.used_viewport_units.store(true, Ordering::Relaxed);
        // Servo doesn't have dynamic UA interfaces that affect the viewport,
        // so we can just ignore the ViewportVariant.
        self.au_viewport_size()
    }

    /// Returns the page box of the page-relative length units in app units,
    /// recording that a page-relative unit participated in computed-value
    /// resolution.
    pub fn au_page_box_size_for_resolution(&self) -> UntypedSize2D<Au> {
        self.used_viewport_units.store(true, Ordering::Relaxed);
        Size2D::new(
            Au::from_f32_px(self.page_unit_box_size.width),
            Au::from_f32_px(self.page_unit_box_size.height),
        )
    }

    /// Returns the bleed-box size of the current device in app units,
    /// used to resolve the moegoe `-bd-b{w,h,i,b,min,max}` length
    /// units (CSS Paged Media L3 §7.1). Mirrors
    /// `au_viewport_size_for_viewport_unit_resolution` but consults
    /// `bleed_box_size` rather than `viewport_size`. When the
    /// embedder has not set the bleed box separately it equals the
    /// viewport, so the bleed-axis arms collapse onto the page-axis
    /// path — preserving the pre-S-A26 fallback semantics.
    pub fn au_bleed_box_size_for_resolution(&self) -> UntypedSize2D<Au> {
        self.used_viewport_units.store(true, Ordering::Relaxed);
        Size2D::new(
            Au::from_f32_px(self.bleed_box_size.width),
            Au::from_f32_px(self.bleed_box_size.height),
        )
    }

    /// Whether viewport units were used since the last device change.
    pub fn used_viewport_units(&self) -> bool {
        self.used_viewport_units.load(Ordering::Relaxed)
    }

    /// Returns the number of app units per device pixel we're using currently.
    pub fn app_units_per_device_pixel(&self) -> i32 {
        (AU_PER_PX as f32 / self.device_pixel_ratio.0) as i32
    }

    /// Returns the device pixel ratio, ignoring the full zoom factor.
    pub fn device_pixel_ratio_ignoring_full_zoom(&self) -> Scale<f32, CSSPixel, DevicePixel> {
        self.device_pixel_ratio
    }

    /// Returns the device pixel ratio.
    pub fn device_pixel_ratio(&self) -> Scale<f32, CSSPixel, DevicePixel> {
        self.device_pixel_ratio
    }

    /// Set a new device pixel ratio on this [`Device`].
    ///
    /// Note that this does not update any associated `Stylist`. For this you must call
    /// `Stylist::media_features_change_changed_style` and
    /// `Stylist::force_stylesheet_origins_dirty`.
    pub fn set_device_pixel_ratio(
        &mut self,
        device_pixel_ratio: Scale<f32, CSSPixel, DevicePixel>,
    ) {
        self.device_pixel_ratio = device_pixel_ratio;
    }

    /// Gets the size of the scrollbar in CSS pixels.
    pub fn scrollbar_inline_size(&self) -> CSSPixelLength {
        // TODO: implement this.
        CSSPixelLength::new(0.0)
    }

    /// Queries font metrics using the [`FontMetricsProvider`] interface.
    pub fn query_font_metrics(
        &self,
        vertical: bool,
        font: &Font,
        base_size: CSSPixelLength,
        flags: QueryFontMetricsFlags,
        track_usage: bool,
    ) -> FontMetrics {
        if track_usage {
            self.used_font_metrics.store(true, Ordering::Relaxed);
        }
        self.font_metrics_provider
            .query_font_metrics(vertical, font, base_size, flags)
    }

    fn ensure_root_font_metrics_updated(&self) {
        let mut guard = self.used_root_font_metrics.write();
        let previously_computed = mem::replace(&mut *guard, true);
        if !previously_computed {
            self.update_root_font_metrics();
        }
    }

    /// Compute the root element's font metrics, and returns a bool indicating whether
    /// the font metrics have changed since the previous restyle.
    pub fn update_root_font_metrics(&self) -> bool {
        let root_style = self.root_style.read();
        let root_effective_zoom = (*root_style).effective_zoom;
        let root_font_size = (*root_style).get_font().clone_font_size().computed_size();

        let root_font_metrics = self.query_font_metrics(
            (*root_style).writing_mode.is_upright(),
            &(*root_style).get_font(),
            root_font_size,
            QueryFontMetricsFlags::USE_USER_FONT_SET
                | QueryFontMetricsFlags::NEEDS_CH
                | QueryFontMetricsFlags::NEEDS_IC,
            /* track_usage = */ false,
        );

        let mut root_font_metrics_changed = false;
        root_font_metrics_changed |= self.set_root_font_metrics_ex(
            root_effective_zoom.unzoom(root_font_metrics.x_height_or_default(root_font_size).px()),
        );
        root_font_metrics_changed |= self.set_root_font_metrics_ch(
            root_effective_zoom.unzoom(
                root_font_metrics
                    .zero_advance_measure_or_default(
                        root_font_size,
                        (*root_style).writing_mode.is_upright(),
                    )
                    .px(),
            ),
        );
        root_font_metrics_changed |= self.set_root_font_metrics_cap(
            root_effective_zoom.unzoom(root_font_metrics.cap_height_or_default().px()),
        );
        root_font_metrics_changed |= self.set_root_font_metrics_ic(
            root_effective_zoom.unzoom(root_font_metrics.ic_width_or_default(root_font_size).px()),
        );

        root_font_metrics_changed
    }

    /// Return the media type of the current device.
    pub fn media_type(&self) -> MediaType {
        self.media_type.clone()
    }

    /// Returns whether document colors are enabled.
    pub fn forced_colors(&self) -> ForcedColors {
        if self.user_preferences.forced_colors { ForcedColors::Active } else { ForcedColors::None }
    }

    /// Replaces preferences, reporting whether the embedder must invalidate media styles.
    pub fn set_user_preferences(&mut self, preferences: UserPreferences) -> bool {
        if self.user_preferences == preferences {
            return false;
        }
        self.user_preferences = preferences;
        true
    }

    /// The raw preferences, before the forced palette determines its contrast level.
    pub fn user_preferences(&self) -> UserPreferences {
        self.user_preferences
    }

    fn contrast_preference(&self) -> PreferenceContrast {
        if self.user_preferences.forced_colors {
            // This device's forced Canvas/CanvasText palette is white/black.
            PreferenceContrast::More
        } else {
            self.user_preferences.contrast
        }
    }

    /// Returns the default background color.
    pub fn default_background_color(&self) -> AbsoluteColor {
        AbsoluteColor::WHITE
    }

    /// Returns the default foreground color.
    pub fn default_color(&self) -> AbsoluteColor {
        AbsoluteColor::BLACK
    }

    /// Set the [`PrefersColorScheme`] value on this [`Device`].
    ///
    /// Note that this does not update any associated `Stylist`. For this you must call
    /// `Stylist::media_features_change_changed_style` and
    /// `Stylist::force_stylesheet_origins_dirty`.
    pub fn set_color_scheme(&mut self, new_color_scheme: PrefersColorScheme) {
        self.prefers_color_scheme = new_color_scheme;
    }

    /// Returns the color scheme of this [`Device`].
    pub fn color_scheme(&self) -> PrefersColorScheme {
        self.user_preferences.color_scheme.unwrap_or(self.prefers_color_scheme)
    }

    /// Sets the parsed page support and reports whether media and system colors need updating.
    pub fn set_page_color_scheme_support(&mut self, support: ColorSchemeFlags) -> bool {
        if self.page_color_scheme_support == support {
            return false;
        }
        self.page_color_scheme_support = support;
        true
    }

    /// The negotiated page scheme, queried by prefers-color-scheme.
    pub fn page_color_scheme(&self) -> PrefersColorScheme {
        if self.is_dark_color_scheme(self.page_color_scheme_support) {
            PrefersColorScheme::Dark
        } else {
            PrefersColorScheme::Light
        }
    }

    /// Set the [`NavigationPhase`] of this [`Device`].
    ///
    /// Note that this does not update any associated `Stylist`. For this you must call
    /// `Stylist::force_stylesheet_origins_dirty`.
    pub fn set_navigation_phase(&mut self, phase: NavigationPhase) {
        self.navigation_phase = phase;
    }

    /// Returns the [`NavigationPhase`] of this [`Device`].
    pub fn navigation_phase(&self) -> NavigationPhase {
        self.navigation_phase
    }

    pub(crate) fn is_dark_color_scheme(&self, color_scheme: ColorSchemeFlags) -> bool {
        if self.user_preferences.forced_colors {
            return false;
        }
        let color_scheme = if color_scheme.contains(ColorSchemeFlags::NORMAL) {
            self.page_color_scheme_support
        } else {
            color_scheme
        };
        let supports_light = color_scheme.contains(ColorSchemeFlags::LIGHT);
        let supports_dark = color_scheme.contains(ColorSchemeFlags::DARK);
        match (supports_light, supports_dark) {
            (false, true) => true,
            (true, true) => self.color_scheme() == PrefersColorScheme::Dark,
            _ => false,
        }
    }

    /// Returns safe area insets
    pub fn safe_area_insets(&self) -> SideOffsets2D<f32, CSSPixel> {
        SideOffsets2D::zero()
    }

    /// Returns true if the given MIME type is supported
    pub fn is_supported_mime_type(&self, mime_type: &str) -> bool {
        match mime_type.parse::<Mime>() {
            Ok(m) => {
                // Keep this in sync with 'image_classifer' from
                // components/net/mime_classifier.rs
                m == mime::IMAGE_BMP
                    || m == mime::IMAGE_GIF
                    || m == mime::IMAGE_PNG
                    || m == mime::IMAGE_JPEG
                    || m == "image/x-icon"
                    || m == "image/webp"
            },
            _ => false,
        }
    }

    /// Return whether the document is a chrome document.
    #[inline]
    pub fn chrome_rules_enabled_for_document(&self) -> bool {
        false
    }
}

/// https://drafts.csswg.org/mediaqueries-4/#width
///
/// For paged media this describes the page box (the full sheet, page
/// area plus margins), so it reads `page_box_size` rather than the
/// viewport (page area). For continuous media the two coincide.
///
/// The CSS-pixel `page_box_size` is read directly rather than routed
/// through app units: the range comparison in
/// `QueryFeatureExpression::matches` evaluates the feature value
/// against the query length in CSS pixels (`f32`), and the query side
/// is *not* quantised to app units. Quantising the page box to `Au`
/// (1/60 px) here would round it to the nearest app unit and so, on an
/// exact boundary such as `min-width: 210mm` against a 210 mm page,
/// drop it just below the query value (≈0.0008 px) and miss the match.
/// Comparing in `f32` keeps both sides at the same precision.
fn eval_width(context: &Context) -> CSSPixelLength {
    CSSPixelLength::new(context.device().page_box_size().width)
}

/// https://drafts.csswg.org/mediaqueries-4/#height
///
/// See [`eval_width`]: the page box, not the page area, in paged media,
/// read directly in CSS pixels to avoid an app-unit rounding mismatch.
fn eval_height(context: &Context) -> CSSPixelLength {
    CSSPixelLength::new(context.device().page_box_size().height)
}

fn eval_orientation(context: &Context, value: Option<Orientation>) -> bool {
    // Paged media: the `orientation` feature describes the page box (Media
    // Queries 4 §6.1), not the page area. `page_box_size` defaults to the
    // viewport for continuous media, so that case is unchanged. Au precision is
    // sufficient here — this is a width-vs-height comparison, not an exact
    // length boundary (unlike `width`/`height`, which read f32 directly).
    let page_box = context.device().page_box_size();
    let au = UntypedSize2D::new(
        Au::from_f32_px(page_box.width),
        Au::from_f32_px(page_box.height),
    );
    Orientation::eval(au, value)
}

fn eval_aspect_ratio(context: &Context) -> Ratio {
    // Paged media: the page box, not the page area (Media Queries 4 §6.1).
    let page_box = context.device().page_box_size();
    Ratio::new(page_box.width, page_box.height)
}

#[derive(Clone, Copy, Debug, FromPrimitive, Parse, ToCss)]
#[repr(u8)]
enum Scan {
    Progressive,
    Interlace,
}

/// https://drafts.csswg.org/mediaqueries-4/#scan
fn eval_scan(_: &Context, _: Option<Scan>) -> bool {
    // Since we doesn't support the 'tv' media type, the 'scan' feature never
    // matches.
    false
}

/// https://drafts.csswg.org/mediaqueries-4/#resolution
fn eval_resolution(context: &Context) -> Resolution {
    Resolution::from_dppx(context.device().device_pixel_ratio.0)
}

/// https://compat.spec.whatwg.org/#css-media-queries-webkit-device-pixel-ratio
fn eval_device_pixel_ratio(context: &Context) -> f32 {
    eval_resolution(context).dppx()
}

fn eval_prefers_color_scheme(context: &Context, query_value: Option<PrefersColorScheme>) -> bool {
    match query_value {
        Some(v) => context.device().user_preferences.color_scheme.unwrap_or_else(|| context.device().page_color_scheme()) == v,
        None => true,
    }
}

/// https://drafts.csswg.org/mediaqueries-4/#color
fn eval_color(_: &Context) -> i32 {
    // Servo targets colour print/PDF output; the device has a non-zero colour
    // depth. Gecko reads the real depth via `Gecko_MediaFeatures_GetColorDepth`,
    // which is unavailable in the Servo build, so report a fixed 8 bits per
    // component.
    8
}

macro_rules! discrete_feature {
    ($name:ident { $($value:ident),+ }, $evaluate:ident($context:ident), $current:expr, $boolean:expr) => {
        #[derive(Clone, Copy, Debug, FromPrimitive, Parse, PartialEq, ToCss)]
        #[repr(u8)]
        enum $name { $($value),+ }

        fn $evaluate($context: &Context, query: Option<$name>) -> bool {
            let current = $current;
            query.map_or($boolean, |value| value == current)
        }
    };
}

#[derive(Clone, Copy, Debug, FromPrimitive, Parse, PartialEq, ToCss)]
#[repr(u8)]
enum ReducedPreference { NoPreference, Reduce }

fn reduced_preference_matches(reduce: bool, query: Option<ReducedPreference>) -> bool {
    query.map_or(reduce, |value| value == if reduce { ReducedPreference::Reduce } else { ReducedPreference::NoPreference })
}

fn eval_reduced_motion(context: &Context, query: Option<ReducedPreference>) -> bool {
    reduced_preference_matches(context.device().user_preferences.reduced_motion, query)
}

fn eval_reduced_transparency(context: &Context, query: Option<ReducedPreference>) -> bool {
    reduced_preference_matches(context.device().user_preferences.reduced_transparency, query)
}

fn eval_reduced_data(context: &Context, query: Option<ReducedPreference>) -> bool {
    reduced_preference_matches(context.device().user_preferences.reduced_data, query)
}

fn eval_contrast_preference(context: &Context, query: Option<PreferenceContrast>) -> bool {
    let current = context.device().contrast_preference();
    query.map_or(current != PreferenceContrast::NoPreference, |value| value == current)
}
discrete_feature!(
    InvertedColors { None, Inverted },
    eval_inverted_colors(context),
    if context.device().user_preferences.inverted_colors { InvertedColors::Inverted } else { InvertedColors::None },
    context.device().user_preferences.inverted_colors
);
discrete_feature!(
    DynamicRange { Standard, High },
    eval_dynamic_range(_context),
    DynamicRange::Standard,
    true
);
discrete_feature!(
    ColorGamut { Srgb, P3, Rec2020 },
    eval_color_gamut(_context),
    ColorGamut::Srgb,
    true
);
discrete_feature!(
    DisplayMode {
        Browser,
        MinimalUi,
        Standalone,
        Fullscreen,
        PictureInPicture
    },
    eval_display_mode(_context),
    DisplayMode::Browser,
    true
);
discrete_feature!(
    OverflowInline { None, Scroll },
    eval_overflow_inline(context),
    if context.device().media_type() == MediaType::print() {
        OverflowInline::None
    } else {
        OverflowInline::Scroll
    },
    context.device().media_type() != MediaType::print()
);
discrete_feature!(
    OverflowBlock { None, Scroll, Paged },
    eval_overflow_block(context),
    if context.device().media_type() == MediaType::print() {
        OverflowBlock::Paged
    } else {
        OverflowBlock::Scroll
    },
    true
);
discrete_feature!(
    Update { None, Slow, Fast },
    eval_update(context),
    if context.device().media_type() == MediaType::print() {
        Update::None
    } else {
        Update::Fast
    },
    context.device().media_type() != MediaType::print()
);
discrete_feature!(
    Scripting {
        None,
        InitialOnly,
        Enabled
    },
    eval_scripting(context),
    if context.device().media_type() == MediaType::print() {
        Scripting::InitialOnly
    } else {
        Scripting::Enabled
    },
    true
);
discrete_feature!(
    Pointer { None, Coarse, Fine },
    eval_pointer(context),
    if context.device().media_type() == MediaType::print() {
        Pointer::None
    } else {
        Pointer::Fine
    },
    context.device().media_type() != MediaType::print()
);
discrete_feature!(
    Hover { None, Hover },
    eval_hover(context),
    if context.device().media_type() == MediaType::print() {
        Hover::None
    } else {
        Hover::Hover
    },
    context.device().media_type() != MediaType::print()
);

fn eval_forced_colors(context: &Context, query: Option<ForcedColors>) -> bool {
    let forced = context.device().forced_colors();
    query.map_or(forced != ForcedColors::None, |value| value == forced)
}

fn eval_zero(_: &Context) -> i32 {
    0
}
fn eval_grid(_: &Context) -> bool {
    false
}
fn eval_device_width(context: &Context) -> CSSPixelLength {
    CSSPixelLength::new(context.device().device_size.width)
}
fn eval_device_height(context: &Context) -> CSSPixelLength {
    CSSPixelLength::new(context.device().device_size.height)
}
fn eval_device_aspect_ratio(context: &Context) -> Ratio {
    let size = context.device().device_size;
    Ratio::new(size.width, size.height)
}

/// A list with all the media features that Servo supports.
pub static MEDIA_FEATURES: [QueryFeatureDescription; 35] = [
    feature!(
        atom!("width"),
        AllowsRanges::Yes,
        Evaluator::Length(eval_width),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("height"),
        AllowsRanges::Yes,
        Evaluator::Length(eval_height),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("orientation"),
        AllowsRanges::No,
        keyword_evaluator!(eval_orientation, Orientation),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("aspect-ratio"),
        AllowsRanges::Yes,
        Evaluator::NumberRatio(eval_aspect_ratio),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("scan"),
        AllowsRanges::No,
        keyword_evaluator!(eval_scan, Scan),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("resolution"),
        AllowsRanges::Yes,
        Evaluator::Resolution(eval_resolution),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("device-pixel-ratio"),
        AllowsRanges::Yes,
        Evaluator::Float(eval_device_pixel_ratio),
        FeatureFlags::WEBKIT_PREFIX,
    ),
    feature!(
        atom!("-moz-device-pixel-ratio"),
        AllowsRanges::Yes,
        Evaluator::Float(eval_device_pixel_ratio),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("prefers-color-scheme"),
        AllowsRanges::No,
        keyword_evaluator!(eval_prefers_color_scheme, PrefersColorScheme),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("color"),
        AllowsRanges::Yes,
        Evaluator::Integer(eval_color),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("device-width"),
        AllowsRanges::Yes,
        Evaluator::Length(eval_device_width),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("device-height"),
        AllowsRanges::Yes,
        Evaluator::Length(eval_device_height),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("device-aspect-ratio"),
        AllowsRanges::Yes,
        Evaluator::NumberRatio(eval_device_aspect_ratio),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("color-index"),
        AllowsRanges::Yes,
        Evaluator::Integer(eval_zero),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("monochrome"),
        AllowsRanges::Yes,
        Evaluator::Integer(eval_zero),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("grid"),
        AllowsRanges::No,
        Evaluator::BoolInteger(eval_grid),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("prefers-reduced-motion"),
        AllowsRanges::No,
        keyword_evaluator!(eval_reduced_motion, ReducedPreference),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("prefers-reduced-transparency"),
        AllowsRanges::No,
        keyword_evaluator!(eval_reduced_transparency, ReducedPreference),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("prefers-reduced-data"),
        AllowsRanges::No,
        keyword_evaluator!(eval_reduced_data, ReducedPreference),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("prefers-contrast"),
        AllowsRanges::No,
        keyword_evaluator!(eval_contrast_preference, PreferenceContrast),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("forced-colors"),
        AllowsRanges::No,
        keyword_evaluator!(eval_forced_colors, ForcedColors),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("inverted-colors"),
        AllowsRanges::No,
        keyword_evaluator!(eval_inverted_colors, InvertedColors),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("color-gamut"),
        AllowsRanges::No,
        keyword_evaluator!(eval_color_gamut, ColorGamut),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("video-color-gamut"),
        AllowsRanges::No,
        keyword_evaluator!(eval_color_gamut, ColorGamut),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("dynamic-range"),
        AllowsRanges::No,
        keyword_evaluator!(eval_dynamic_range, DynamicRange),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("video-dynamic-range"),
        AllowsRanges::No,
        keyword_evaluator!(eval_dynamic_range, DynamicRange),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("display-mode"),
        AllowsRanges::No,
        keyword_evaluator!(eval_display_mode, DisplayMode),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("overflow-inline"),
        AllowsRanges::No,
        keyword_evaluator!(eval_overflow_inline, OverflowInline),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("overflow-block"),
        AllowsRanges::No,
        keyword_evaluator!(eval_overflow_block, OverflowBlock),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("update"),
        AllowsRanges::No,
        keyword_evaluator!(eval_update, Update),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("scripting"),
        AllowsRanges::No,
        keyword_evaluator!(eval_scripting, Scripting),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("pointer"),
        AllowsRanges::No,
        keyword_evaluator!(eval_pointer, Pointer),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("any-pointer"),
        AllowsRanges::No,
        keyword_evaluator!(eval_pointer, Pointer),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("hover"),
        AllowsRanges::No,
        keyword_evaluator!(eval_hover, Hover),
        FeatureFlags::empty(),
    ),
    feature!(
        atom!("any-hover"),
        AllowsRanges::No,
        keyword_evaluator!(eval_hover, Hover),
        FeatureFlags::empty(),
    ),
];

#[cfg(test)]
mod page_color_scheme_support_tests {
    use super::*;

    #[test]
    fn explicit_color_override_controls_queries_and_used_scheme_without_changing_page_support() {
        let mut device = crate::test_support::test_device();
        let support = device.page_color_scheme_support;
        device.set_user_preferences(UserPreferences { color_scheme: Some(PrefersColorScheme::Dark), ..Default::default() });
        assert_eq!(device.color_scheme(), PrefersColorScheme::Dark);
        assert_eq!(device.page_color_scheme_support, support);
        Context::for_media_query_evaluation(&device, QuirksMode::NoQuirks, |context| {
            assert!(eval_prefers_color_scheme(context, Some(PrefersColorScheme::Dark)));
            assert!(!eval_prefers_color_scheme(context, Some(PrefersColorScheme::Light)));
        });
        assert!(device.is_dark_color_scheme(ColorSchemeFlags::LIGHT | ColorSchemeFlags::DARK));
        device.set_user_preferences(UserPreferences::default());
        assert_eq!(device.color_scheme(), PrefersColorScheme::Light);
        Context::for_media_query_evaluation(&device, QuirksMode::NoQuirks, |context| {
            assert!(eval_prefers_color_scheme(context, Some(PrefersColorScheme::Light)));
        });
    }

    #[test]
    fn reduced_preferences_are_independent_and_resettable() {
        let mut device = crate::test_support::test_device();
        assert!(!device.set_user_preferences(UserPreferences::default()));
        for (preferences, expected) in [
            (UserPreferences { reduced_motion: true, ..Default::default() }, [true, false, false]),
            (UserPreferences { reduced_transparency: true, ..Default::default() }, [false, true, false]),
            (UserPreferences { reduced_data: true, ..Default::default() }, [false, false, true]),
            (UserPreferences::default(), [false, false, false]),
        ] {
            assert!(device.set_user_preferences(preferences));
            assert_eq!(device.user_preferences(), preferences);
            assert!(!device.set_user_preferences(preferences));
            Context::for_media_query_evaluation(&device, QuirksMode::NoQuirks, |context| {
                let evaluators: [fn(&Context, Option<ReducedPreference>) -> bool; 3] = [
                    eval_reduced_motion, eval_reduced_transparency, eval_reduced_data,
                ];
                for (evaluate, reduce) in evaluators.into_iter().zip(expected) {
                    assert_eq!(evaluate(context, None), reduce);
                    assert_eq!(evaluate(context, Some(ReducedPreference::Reduce)), reduce);
                    assert_eq!(evaluate(context, Some(ReducedPreference::NoPreference)), !reduce);
                }
            });
        }
    }

    #[test]
    fn contrast_and_output_preferences_keep_raw_color_preference_separate() {
        let mut device = crate::test_support::test_device();
        device.set_color_scheme(PrefersColorScheme::Dark);
        device.set_page_color_scheme_support(ColorSchemeFlags::DARK);
        for contrast in [PreferenceContrast::More, PreferenceContrast::Less, PreferenceContrast::Custom, PreferenceContrast::NoPreference] {
            device.set_user_preferences(UserPreferences { contrast, ..Default::default() });
            Context::for_media_query_evaluation(&device, QuirksMode::NoQuirks, |context| {
                assert_eq!(eval_contrast_preference(context, None), contrast != PreferenceContrast::NoPreference);
                assert!(eval_contrast_preference(context, Some(contrast)));
                assert!(!eval_inverted_colors(context, None));
                assert!(!eval_forced_colors(context, None));
            });
            assert_eq!(device.page_color_scheme(), PrefersColorScheme::Dark);
        }
        device.set_user_preferences(UserPreferences { forced_colors: true, inverted_colors: true, ..Default::default() });
        Context::for_media_query_evaluation(&device, QuirksMode::NoQuirks, |context| {
            assert!(eval_contrast_preference(context, Some(PreferenceContrast::More)));
            assert!(eval_inverted_colors(context, Some(InvertedColors::Inverted)));
            assert!(eval_forced_colors(context, Some(ForcedColors::Active)));
        });
        assert_eq!(device.page_color_scheme(), PrefersColorScheme::Light);
        assert_eq!(device.color_scheme(), PrefersColorScheme::Dark);
        device.set_user_preferences(UserPreferences::default());
        assert_eq!(device.page_color_scheme(), PrefersColorScheme::Dark);
    }

    #[test]
    fn setting_page_support_reports_only_changes_and_retains_the_user_preference() {
        let mut device = crate::test_support::test_device();
        device.set_color_scheme(PrefersColorScheme::Dark);
        assert!(!device.set_page_color_scheme_support(ColorSchemeFlags::empty()));
        assert!(device.set_page_color_scheme_support(ColorSchemeFlags::DARK));
        assert_eq!(device.page_color_scheme_support, ColorSchemeFlags::DARK);
        assert!(!device.set_page_color_scheme_support(ColorSchemeFlags::DARK));
        assert!(device.set_page_color_scheme_support(ColorSchemeFlags::LIGHT | ColorSchemeFlags::DARK));
        assert_eq!(device.color_scheme(), PrefersColorScheme::Dark);
    }
}
