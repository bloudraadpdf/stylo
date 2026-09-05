/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Common handling for the specified value CSS url() values.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::stylesheets::CorsMode;
use crate::values::computed::{Context, ToComputedValue};
use cssparser::{match_ignore_ascii_case, Parser};
use servo_arc::Arc;
use std::fmt::{self, Write};
use std::ops::Deref;
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};
use to_shmem::{SharedMemoryBuilder, ToShmem};
use url::Url;

/// A CSS url() value for servo.
///
/// Servo eagerly resolves SpecifiedUrls, which it can then take advantage of
/// when computing values. In contrast, Gecko uses a different URL backend, so
/// eagerly resolving with rust-url would be duplicated work.
///
/// However, this approach is still not necessarily optimal: See
/// <https://bugzilla.mozilla.org/show_bug.cgi?id=1347435#c6>
#[derive(Clone, Debug, Deserialize, MallocSizeOf, Serialize, SpecifiedValueInfo)]
#[css(function = "url")]
#[repr(C)]
pub struct CssUrl(#[ignore_malloc_size_of = "Arc"] pub Arc<CssUrlData>);

/// Data shared between CssUrls.
///
#[derive(Debug, Deserialize, MallocSizeOf, Serialize, SpecifiedValueInfo)]
#[repr(C)]
pub struct CssUrlData {
    /// The original URI. This might be optional since we may insert computed
    /// values of images into the cascade directly, and we don't bother to
    /// convert their serialization.
    ///
    /// Refcounted since cloning this should be cheap and data: uris can be
    /// really large.
    #[ignore_malloc_size_of = "Arc"]
    original: Option<Arc<String>>,

    /// The resolved value for the url, if valid.
    #[ignore_malloc_size_of = "Arc"]
    resolved: Option<Arc<Url>>,

    /// The property-selected CORS mode for this URL.
    #[css(skip)]
    cors_mode: CorsMode,

    /// Request modifiers authored as part of the quoted `url()` value.
    #[css(skip)]
    modifiers: UrlRequestModifiers,
}

/// The CORS credentials mode selected by a CSS Values 5 `cross-origin()` URL
/// request modifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize)]
pub enum UrlCorsMode {
    /// `cross-origin(anonymous)`.
    Anonymous,
    /// `cross-origin(use-credentials)`.
    UseCredentials,
}

/// A CSS Values 5 referrer policy URL request modifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize)]
pub enum UrlReferrerPolicy {
    /// Send no referrer.
    NoReferrer,
    /// Send no referrer on a downgrade.
    NoReferrerWhenDowngrade,
    /// Send a referrer only for same-origin requests.
    SameOrigin,
    /// Send only the origin.
    Origin,
    /// Send only the origin, except on a downgrade.
    StrictOrigin,
    /// Send the full URL for same-origin requests and the origin otherwise.
    OriginWhenCrossOrigin,
    /// Apply strict-origin behavior to cross-origin requests.
    StrictOriginWhenCrossOrigin,
    /// Always send the full URL as the referrer.
    UnsafeUrl,
}

/// Canonical, typed URL request modifiers.
///
/// CSS Values 5 permits each modifier at most once and serializes them in
/// cross-origin, integrity, referrer-policy order. Keeping separate fields
/// makes duplicate modifiers unrepresentable after parsing and prevents one
/// request setting from being confused with another.
#[derive(Clone, Debug, Default, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub struct UrlRequestModifiers {
    cors: Option<UrlCorsMode>,
    integrity: Option<String>,
    referrer_policy: Option<UrlReferrerPolicy>,
}

impl UrlRequestModifiers {
    /// Return the explicitly authored CORS modifier.
    pub fn cors(&self) -> Option<UrlCorsMode> {
        self.cors
    }

    /// Return the explicitly authored integrity metadata.
    pub fn integrity(&self) -> Option<&str> {
        self.integrity.as_deref()
    }

    /// Return the explicitly authored referrer policy modifier.
    pub fn referrer_policy(&self) -> Option<UrlReferrerPolicy> {
        self.referrer_policy
    }

    /// Parse the request modifiers after a URL string.
    pub fn parse<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i>> {
        let mut modifiers = Self::default();
        while !input.is_exhausted() {
            let name = input.expect_function()?.clone();
            match_ignore_ascii_case! { &name,
                "cross-origin" if modifiers.cors.is_none() => {
                    modifiers.cors = Some(input.parse_nested_block(|input| {
                        let mode = if input.try_parse(|input| input.expect_ident_matching("anonymous")).is_ok() {
                            UrlCorsMode::Anonymous
                        } else {
                            input.expect_ident_matching("use-credentials")?;
                            UrlCorsMode::UseCredentials
                        };
                        input.expect_exhausted()?;
                        Ok(mode)
                    })?);
                },
                "integrity" if modifiers.integrity.is_none() => {
                    modifiers.integrity = Some(input.parse_nested_block(|input| {
                        let value = input.expect_string()?.as_ref().to_owned();
                        input.expect_exhausted()?;
                        Ok(value)
                    })?);
                },
                "referrer-policy" if modifiers.referrer_policy.is_none() => {
                    modifiers.referrer_policy = Some(input.parse_nested_block(|input| {
                        let ident = input.expect_ident_cloned()?;
                        input.expect_exhausted()?;
                        match_ignore_ascii_case! { ident.as_ref(),
                            "no-referrer" => Ok(UrlReferrerPolicy::NoReferrer),
                            "no-referrer-when-downgrade" => Ok(UrlReferrerPolicy::NoReferrerWhenDowngrade),
                            "same-origin" => Ok(UrlReferrerPolicy::SameOrigin),
                            "origin" => Ok(UrlReferrerPolicy::Origin),
                            "strict-origin" => Ok(UrlReferrerPolicy::StrictOrigin),
                            "origin-when-cross-origin" => Ok(UrlReferrerPolicy::OriginWhenCrossOrigin),
                            "strict-origin-when-cross-origin" => Ok(UrlReferrerPolicy::StrictOriginWhenCrossOrigin),
                            "unsafe-url" => Ok(UrlReferrerPolicy::UnsafeUrl),
                            _ => Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
                        }
                    })?);
                },
                _ => return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
            }
        }
        Ok(modifiers)
    }

    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if let Some(cors) = self.cors {
            dest.write_str(" cross-origin(")?;
            dest.write_str(match cors {
                UrlCorsMode::Anonymous => "anonymous",
                UrlCorsMode::UseCredentials => "use-credentials",
            })?;
            dest.write_char(')')?;
        }
        if let Some(ref integrity) = self.integrity {
            dest.write_str(" integrity(")?;
            integrity.to_css(dest)?;
            dest.write_char(')')?;
        }
        if let Some(policy) = self.referrer_policy {
            dest.write_str(" referrer-policy(")?;
            dest.write_str(match policy {
                UrlReferrerPolicy::NoReferrer => "no-referrer",
                UrlReferrerPolicy::NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
                UrlReferrerPolicy::SameOrigin => "same-origin",
                UrlReferrerPolicy::Origin => "origin",
                UrlReferrerPolicy::StrictOrigin => "strict-origin",
                UrlReferrerPolicy::OriginWhenCrossOrigin => "origin-when-cross-origin",
                UrlReferrerPolicy::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
                UrlReferrerPolicy::UnsafeUrl => "unsafe-url",
            })?;
            dest.write_char(')')?;
        }
        Ok(())
    }
}

impl ToShmem for CssUrl {
    fn to_shmem(&self, _builder: &mut SharedMemoryBuilder) -> to_shmem::Result<Self> {
        unimplemented!("If servo wants to share stylesheets across processes, ToShmem for Url must be implemented");
    }
}

impl Deref for CssUrl {
    type Target = CssUrlData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CssUrl {
    /// Try to parse a URL from a string value that is a valid CSS token for a
    /// URL.
    ///
    pub fn parse_from_string(url: String, context: &ParserContext, cors_mode: CorsMode) -> Self {
        let serialization = Arc::new(url);
        let resolved = context.url_data.0.join(&serialization).ok().map(Arc::new);
        CssUrl(Arc::new(CssUrlData {
            original: Some(serialization),
            resolved: resolved,
            cors_mode,
            modifiers: UrlRequestModifiers::default(),
        }))
    }

    /// Returns true if the URL is definitely invalid. For Servo URLs, we can
    /// use its |resolved| status.
    pub fn is_invalid(&self) -> bool {
        self.resolved.is_none()
    }

    /// Returns true if this URL looks like a fragment.
    /// See https://drafts.csswg.org/css-values/#local-urls
    ///
    /// Since Servo currently stores resolved URLs, this is hard to implement. We
    /// either need to change servo to lazily resolve (like Gecko), or note this
    /// information in the tokenizer.
    pub fn is_fragment(&self) -> bool {
        error!("Can't determine whether the url is a fragment.");
        false
    }

    /// Returns the resolved url if it was valid.
    pub fn url(&self) -> Option<&Arc<Url>> {
        self.resolved.as_ref()
    }

    /// Return the resolved url as string, or the empty string if it's invalid.
    ///
    /// TODO(emilio): Should we return the original one if needed?
    pub fn as_str(&self) -> &str {
        match self.resolved {
            Some(ref url) => url.as_str(),
            None => "",
        }
    }

    /// Return the authored URL text before base-URL resolution, when present.
    pub fn original(&self) -> Option<&str> {
        self.original.as_deref().map(|value| value.as_str())
    }

    /// Return the canonical request modifiers authored on this URL.
    pub fn request_modifiers(&self) -> &UrlRequestModifiers {
        &self.modifiers
    }

    /// Return the CORS mode selected by the property that parsed this URL.
    pub fn cors_mode(&self) -> CorsMode {
        self.cors_mode
    }

    /// Creates an already specified url value from an already resolved URL
    /// for insertion in the cascade.
    pub fn for_cascade(url: Arc<::url::Url>) -> Self {
        CssUrl(Arc::new(CssUrlData {
            original: None,
            resolved: Some(url),
            cors_mode: CorsMode::None,
            modifiers: UrlRequestModifiers::default(),
        }))
    }

    /// Gets a new url from a string for unit tests.
    pub fn new_for_testing(url: &str) -> Self {
        CssUrl(Arc::new(CssUrlData {
            original: Some(Arc::new(url.into())),
            resolved: ::url::Url::parse(url).ok().map(Arc::new),
            cors_mode: CorsMode::None,
            modifiers: UrlRequestModifiers::default(),
        }))
    }

    /// Parses a URL request and records that the corresponding request needs to
    /// be CORS-enabled.
    ///
    /// This is only for shape images and masks in Gecko, thus unimplemented for
    /// now so somebody notices when trying to do so.
    pub fn parse_with_cors_mode<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        cors_mode: CorsMode,
    ) -> Result<Self, ParseError<'i>> {
        let before = input.state();
        if let Ok(url) = input.try_parse(|input| input.expect_url()) {
            return Ok(Self::parse_from_string(
                url.as_ref().to_owned(),
                context,
                cors_mode,
            ));
        }
        input.reset(&before);
        input.expect_function_matching("url")?;
        let (url, modifiers) = input.parse_nested_block(|input| {
            let url = input.expect_string()?.as_ref().to_owned();
            let modifiers = UrlRequestModifiers::parse(input)?;
            Ok((url, modifiers))
        })?;
        let mut parsed = Self::parse_from_string(url, context, cors_mode);
        Arc::get_mut(&mut parsed.0)
            .expect("a freshly parsed CSS URL has a unique data allocation")
            .modifiers = modifiers;
        Ok(parsed)
    }
}

impl Parse for CssUrl {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_cors_mode(context, input, CorsMode::None)
    }
}

impl PartialEq for CssUrl {
    fn eq(&self, other: &Self) -> bool {
        self.resolved == other.resolved && self.cors_mode == other.cors_mode
    }
}

impl Eq for CssUrl {}

impl ToCss for CssUrl {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let string = match self.0.original {
            Some(ref original) => &**original,
            None => match self.resolved {
                Some(ref url) => url.as_str(),
                // This can only happen if the url wasn't specified by the
                // user *and* it's an invalid url that has been transformed
                // back to specified value via the "uncompute" functionality.
                None => "about:invalid",
            },
        };

        dest.write_str("url(")?;
        string.to_css(dest)?;
        self.modifiers.to_css(dest)?;
        dest.write_char(')')
    }
}

/// A specified url() value for servo.
pub type SpecifiedUrl = CssUrl;

impl ToComputedValue for SpecifiedUrl {
    type ComputedValue = ComputedUrl;

    // If we can't resolve the URL from the specified one, we fall back to the original
    // but still return it as a ComputedUrl::Invalid
    fn to_computed_value(&self, _: &Context) -> Self::ComputedValue {
        match self.resolved {
            Some(ref url) => ComputedUrl::Valid(Arc::new(ValidComputedUrl {
                url: url.clone(),
                cors_mode: self.cors_mode,
                modifiers: self.modifiers.clone(),
            })),
            None => match self.original {
                Some(ref url) => ComputedUrl::Invalid(Arc::new(InvalidComputedUrl {
                    serialization: url.clone(),
                    cors_mode: self.cors_mode,
                    modifiers: self.modifiers.clone(),
                })),
                None => {
                    unreachable!("Found specified url with neither resolved or original URI!");
                },
            },
        }
    }

    fn from_computed_value(computed: &ComputedUrl) -> Self {
        let data = match computed {
            ComputedUrl::Valid(computed) => CssUrlData {
                original: None,
                resolved: Some(computed.url.clone()),
                cors_mode: computed.cors_mode,
                modifiers: computed.modifiers.clone(),
            },
            ComputedUrl::Invalid(computed) => CssUrlData {
                original: Some(computed.serialization.clone()),
                resolved: None,
                cors_mode: computed.cors_mode,
                modifiers: computed.modifiers.clone(),
            },
        };
        CssUrl(Arc::new(data))
    }
}

/// The computed value of a CSS `url()`, resolved relative to the stylesheet URL.
#[derive(Clone, Debug, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub enum ComputedUrl {
    /// A URL that could not be resolved, with its request modifiers retained.
    Invalid(#[ignore_malloc_size_of = "Arc"] Arc<InvalidComputedUrl>),
    /// A resolved URL with its request modifiers retained.
    Valid(#[ignore_malloc_size_of = "Arc"] Arc<ValidComputedUrl>),
}

/// Private payload of an invalid computed URL.
#[derive(Debug, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub struct InvalidComputedUrl {
    #[ignore_malloc_size_of = "Arc"]
    serialization: Arc<String>,
    cors_mode: CorsMode,
    modifiers: UrlRequestModifiers,
}

/// Private payload of a valid computed URL.
#[derive(Debug, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub struct ValidComputedUrl {
    #[ignore_malloc_size_of = "Arc"]
    url: Arc<Url>,
    cors_mode: CorsMode,
    modifiers: UrlRequestModifiers,
}

impl ComputedUrl {
    /// Returns the resolved url if it was valid.
    pub fn url(&self) -> Option<&Arc<Url>> {
        match self {
            ComputedUrl::Valid(computed) => Some(&computed.url),
            ComputedUrl::Invalid(_) => None,
        }
    }

    /// Return the canonical request modifiers attached to this URL.
    pub fn request_modifiers(&self) -> &UrlRequestModifiers {
        match self {
            ComputedUrl::Valid(computed) => &computed.modifiers,
            ComputedUrl::Invalid(computed) => &computed.modifiers,
        }
    }

    /// Return the CORS mode selected by the property that parsed this URL.
    pub fn cors_mode(&self) -> CorsMode {
        match self {
            ComputedUrl::Valid(computed) => computed.cors_mode,
            ComputedUrl::Invalid(computed) => computed.cors_mode,
        }
    }

    /// Return the original serialization when this URL could not be resolved.
    pub fn invalid_serialization(&self) -> Option<&str> {
        match self {
            ComputedUrl::Invalid(computed) => Some(&computed.serialization),
            ComputedUrl::Valid(_) => None,
        }
    }
}

impl ToCss for ComputedUrl {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let string = match self {
            ComputedUrl::Valid(computed) => computed.url.as_str(),
            ComputedUrl::Invalid(computed) => computed.serialization.as_str(),
        };

        dest.write_str("url(")?;
        string.to_css(dest)?;
        self.request_modifiers().to_css(dest)?;
        dest.write_char(')')
    }
}
