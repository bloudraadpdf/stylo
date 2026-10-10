/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared test helpers for Servo-only Stylo unit tests.

use std::sync::{Mutex, OnceLock};

use crate::context::QuirksMode;
use crate::font_metrics::FontMetrics;
use crate::media_queries::MediaList;
use crate::media_queries::MediaType;
use crate::properties::{style_structs::Font, ComputedValues};
use crate::queries::values::PrefersColorScheme;
use crate::servo::media_queries::{Device, FontMetricsProvider};
use crate::shared_lock::SharedRwLock;
use crate::stylesheets::{AllowImportRules, Origin, Stylesheet, UrlExtraData};
use crate::values::computed::font::GenericFontFamily;
use crate::values::computed::{CSSPixelLength, Context, Length};
use euclid::{Scale, Size2D};
use style_traits::{CSSPixel, DevicePixel};

/// Serialises tests that mutate global style prefs.
pub(crate) fn pref_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Restores a bool pref to its previous value when the guard is dropped.
pub(crate) struct BoolPrefGuard {
    key: &'static str,
    old: bool,
}

impl BoolPrefGuard {
    /// Set a bool pref for the lifetime of the returned guard.
    pub(crate) fn set(key: &'static str, value: bool) -> Self {
        let old = style_config::get_bool(key);
        style_config::set_bool(key, value);
        Self { key, old }
    }
}

impl Drop for BoolPrefGuard {
    fn drop(&mut self) {
        style_config::set_bool(self.key, self.old);
    }
}

#[derive(Debug)]
struct TestFontMetricsProvider;

impl FontMetricsProvider for TestFontMetricsProvider {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        _font: &Font,
        _base_size: CSSPixelLength,
        _flags: crate::values::specified::font::QueryFontMetricsFlags,
    ) -> FontMetrics {
        FontMetrics::default()
    }

    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

/// An 800x600 print device.
pub(crate) fn test_device() -> Device {
    Device::new(
        MediaType::print(),
        QuirksMode::NoQuirks,
        Size2D::<f32, CSSPixel>::new(800.0, 600.0),
        Scale::<f32, CSSPixel, DevicePixel>::new(1.0),
        Box::new(TestFontMetricsProvider),
        ComputedValues::initial_values_with_font_override(Font::initial_values()),
        PrefersColorScheme::Light,
    )
}

/// Evaluates with a computed-value context of the test device.
pub(crate) fn with_computed_context<R>(evaluate: impl FnOnce(&Context) -> R) -> R {
    Context::for_media_query_evaluation(&test_device(), QuirksMode::NoQuirks, evaluate)
}

/// Parses an author stylesheet.
pub(crate) fn parse_stylesheet(css: &str) -> Stylesheet {
    let shared_lock = SharedRwLock::new();
    let media = servo_arc::Arc::new(shared_lock.wrap(MediaList::empty()));
    let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
    Stylesheet::from_str(
        css,
        url_data,
        Origin::Author,
        media,
        shared_lock,
        None,
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::Yes,
    )
}
