use std::sync::LazyLock;

use selectors::matching::QuirksMode;
use servo_arc::Arc;
use style::media_queries::MediaList;
use style::shared_lock::SharedRwLock;
use style::stylesheets::{AllowImportRules, Origin, Stylesheet};

pub use crate::preferences::initialise_required_servo_style_prefs;

pub static ABOUT_BLANK: LazyLock<url::Url> =
    LazyLock::new(|| url::Url::parse("about:blank").expect("the built-in base URL is valid"));

static SHARED_LOCK: LazyLock<SharedRwLock> = LazyLock::new(SharedRwLock::new_leaked);

pub fn global_shared_lock() -> &'static SharedRwLock {
    &SHARED_LOCK
}

pub fn parse_stylesheet_fragment(
    css: &str,
    origin: Origin,
) -> (std::sync::Arc<Stylesheet>, std::sync::Arc<SharedRwLock>) {
    let url_data = style::stylesheets::UrlExtraData::from(ABOUT_BLANK.clone());
    parse_stylesheet_fragment_with_url_data(css, origin, url_data)
}

pub fn parse_stylesheet_fragment_with_url_data(
    css: &str,
    origin: Origin,
    url_data: style::stylesheets::UrlExtraData,
) -> (std::sync::Arc<Stylesheet>, std::sync::Arc<SharedRwLock>) {
    let lock = std::sync::Arc::new(SharedRwLock::new());
    let loader = crate::authored_rules::NonLoadingImportLoader;
    let media = Arc::new(lock.wrap(MediaList::empty()));
    let stylesheet = Stylesheet::from_str(
        css,
        url_data,
        origin,
        media,
        (*lock).clone(),
        Some(&loader),
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::Yes,
    );
    (std::sync::Arc::new(stylesheet), lock)
}
