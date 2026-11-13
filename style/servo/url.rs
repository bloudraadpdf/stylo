/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Common handling for the specified value CSS url() values.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::stylesheets::CorsMode;
use crate::values::computed::{Context, ToComputedValue};
use cssparser::{Parser, match_ignore_ascii_case};
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
#[derive(Deserialize, MallocSizeOf, SpecifiedValueInfo)]
#[repr(C)]
pub struct CssUrlData {
    /// Authored URI text when the resolved serialization cannot be used.
    /// Refcounted so cloning a non-canonical URI stays cheap.
    #[ignore_malloc_size_of = "Arc"]
    original: Option<Arc<String>>,

    #[ignore_malloc_size_of = "Arc"]
    referrer_source: Option<Arc<Url>>,

    /// The resolved value for the url, if valid.
    #[ignore_malloc_size_of = "Arc"]
    resolved: Option<Arc<Url>>,

    /// The property-selected CORS mode for this URL.
    #[css(skip)]
    cors_mode: CorsMode,

    /// The authored URI has the resolved URL's exact serialization.
    #[css(skip)]
    #[serde(skip)]
    original_from_resolved: bool,

    /// Request modifiers authored as part of the quoted `url()` value.
    #[css(skip)]
    modifiers: UrlRequestModifiers,
}

impl CssUrlData {
    fn original(&self) -> Option<&str> {
        self.original
            .as_deref()
            .map(|value| value.as_str())
            .or_else(|| {
                if self.original_from_resolved {
                    self.resolved.as_ref().map(|url| url.as_str())
                } else {
                    None
                }
            })
    }
}

impl fmt::Debug for CssUrlData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CssUrlData")
            .field("original", &self.original())
            .field("referrer_source", &self.referrer_source)
            .field("resolved", &self.resolved)
            .field("cors_mode", &self.cors_mode)
            .field("modifiers", &self.modifiers)
            .finish()
    }
}

impl serde::Serialize for CssUrlData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut data = serializer.serialize_struct("CssUrlData", 5)?;
        data.serialize_field("original", &self.original())?;
        data.serialize_field("referrer_source", &self.referrer_source)?;
        data.serialize_field("resolved", &self.resolved)?;
        data.serialize_field("cors_mode", &self.cors_mode)?;
        data.serialize_field("modifiers", &self.modifiers)?;
        data.end()
    }
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
        unimplemented!(
            "If servo wants to share stylesheets across processes, ToShmem for Url must be implemented"
        );
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
        let resolved = if url.is_empty() {
            None
        } else {
            context.url_data.0.join(&url).ok().map(Arc::new)
        };
        let original_from_resolved = resolved
            .as_ref()
            .is_some_and(|resolved| resolved.as_str() == url.as_str());
        let original = if original_from_resolved {
            None
        } else {
            Some(Arc::new(url))
        };
        CssUrl(Arc::new(CssUrlData {
            original,
            referrer_source: context.url_data.referrer_source().cloned(),
            resolved: resolved,
            cors_mode,
            original_from_resolved,
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
    pub fn is_fragment(&self) -> bool {
        self.original()
            .is_some_and(|original| original.starts_with('#'))
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
        self.0.original()
    }

    /// The external stylesheet response URL that supplied this URL value.
    pub fn referrer_source(&self) -> Option<&str> {
        self.0
            .referrer_source
            .as_ref()
            .map(|source| source.as_str())
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
            referrer_source: None,
            resolved: Some(url),
            cors_mode: CorsMode::None,
            original_from_resolved: false,
            modifiers: UrlRequestModifiers::default(),
        }))
    }

    /// Gets a new url from a string for unit tests.
    pub fn new_for_testing(url: &str) -> Self {
        CssUrl(Arc::new(CssUrlData {
            original: Some(Arc::new(url.into())),
            referrer_source: None,
            resolved: ::url::Url::parse(url).ok().map(Arc::new),
            cors_mode: CorsMode::None,
            original_from_resolved: false,
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
        input.skip_whitespace();
        context.reject_attr_tainted_url(input, input.position())?;
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
        self.resolved == other.resolved
            && self.cors_mode == other.cors_mode
            && self.modifiers == other.modifiers
            && self.referrer_source == other.referrer_source
            && self.is_fragment() == other.is_fragment()
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
                referrer_source: self.0.referrer_source.clone(),
                local: self
                    .original
                    .as_ref()
                    .filter(|url| url.starts_with('#'))
                    .cloned(),
                cors_mode: self.cors_mode,
                modifiers: self.modifiers.clone(),
            })),
            None => match self.original {
                Some(ref url) => ComputedUrl::Invalid(Arc::new(InvalidComputedUrl {
                    serialization: url.clone(),
                    referrer_source: self.0.referrer_source.clone(),
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
                original: computed.local.clone(),
                referrer_source: computed.referrer_source.clone(),
                resolved: Some(computed.url.clone()),
                cors_mode: computed.cors_mode,
                original_from_resolved: false,
                modifiers: computed.modifiers.clone(),
            },
            ComputedUrl::Invalid(computed) => CssUrlData {
                original: Some(computed.serialization.clone()),
                referrer_source: computed.referrer_source.clone(),
                resolved: None,
                cors_mode: computed.cors_mode,
                original_from_resolved: false,
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
    #[ignore_malloc_size_of = "Arc"]
    referrer_source: Option<Arc<Url>>,
    cors_mode: CorsMode,
    modifiers: UrlRequestModifiers,
}

/// Private payload of a valid computed URL.
#[derive(Debug, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub struct ValidComputedUrl {
    #[ignore_malloc_size_of = "Arc"]
    url: Arc<Url>,
    #[ignore_malloc_size_of = "Arc"]
    referrer_source: Option<Arc<Url>>,
    #[ignore_malloc_size_of = "Arc"]
    local: Option<Arc<String>>,
    cors_mode: CorsMode,
    modifiers: UrlRequestModifiers,
}

impl ComputedUrl {
    /// The external stylesheet response URL that supplied this URL value.
    pub fn referrer_source(&self) -> Option<&str> {
        match self {
            Self::Valid(computed) => computed.referrer_source.as_ref(),
            Self::Invalid(computed) => computed.referrer_source.as_ref(),
        }
        .map(|source| source.as_str())
    }

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
            ComputedUrl::Valid(computed) => computed
                .local
                .as_ref()
                .map_or_else(|| computed.url.as_str(), |local| local.as_str()),
            ComputedUrl::Invalid(computed) => computed.serialization.as_str(),
        };

        dest.write_str("url(")?;
        string.to_css(dest)?;
        self.request_modifiers().to_css(dest)?;
        dest.write_char(')')
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::CssUrl;
    use crate::custom_properties::AttrTaintedRange;
    use crate::parser::Parse;
    use crate::test_support::{author_parser_context, test_url_data, with_computed_context};
    use crate::values::computed::ToComputedValue;
    use crate::values::specified::Image;
    use cssparser::{Parser, ParserInput};
    use style_traits::ToCss;

    #[test]
    fn canonical_data_url_reuses_resolved_string_storage() {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        let text = format!("data:text/plain,{}", "a".repeat(128 * 1024));
        let specified =
            CssUrl::parse_from_string(text.clone(), &context, crate::stylesheets::CorsMode::None);
        assert_eq!(std::mem::size_of::<CssUrl>(), std::mem::size_of::<usize>());
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<super::CssUrlData>(), 64);
        assert!(specified.0.original.is_none());
        assert_eq!(specified.original(), Some(text.as_str()));
        assert_eq!(specified.as_str(), text);
        assert_eq!(
            specified.original().unwrap().as_ptr(),
            specified.url().unwrap().as_str().as_ptr()
        );
    }

    #[test]
    fn canonical_storage_keeps_authored_spelling_and_constructor_presence() {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        for text in [
            "image.png",
            "../image.png",
            "#local",
            "",
            "http://[",
            "https://EXAMPLE.test:443/a/../b",
            "https://example.test/%7e",
        ] {
            let specified = CssUrl::parse_from_string(
                text.into(),
                &context,
                crate::stylesheets::CorsMode::None,
            );
            assert_eq!(specified.original(), Some(text));
            let mut expected = String::from("url(");
            cssparser::serialize_string(text, &mut expected).unwrap();
            expected.push(')');
            assert_eq!(specified.to_css_string(), expected);
            if !specified.is_invalid() && text != specified.as_str() {
                assert_ne!(
                    specified.original().unwrap().as_ptr(),
                    specified.as_str().as_ptr()
                );
            }
        }
        let testing = CssUrl::new_for_testing("https://example.test/");
        assert_eq!(testing.original(), Some("https://example.test/"));
        let cascade = CssUrl::for_cascade(testing.url().unwrap().clone());
        assert_eq!(cascade.original(), None);
        assert_eq!(cascade.to_css_string(), testing.to_css_string());
    }

    #[test]
    fn canonical_storage_preserves_computed_roundtrips() {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        for text in [
            "data:text/plain,canonical",
            "https://example.test/image.png",
            "image.png",
            "#local",
            "",
            "http://[",
        ] {
            let specified = CssUrl::parse_from_string(
                text.into(),
                &context,
                crate::stylesheets::CorsMode::None,
            );
            with_computed_context(|context| {
                let computed = specified.to_computed_value(context);
                let restored = CssUrl::from_computed_value(&computed);
                assert_eq!(restored, specified);
                assert_eq!(restored.is_invalid(), specified.is_invalid());
                assert_eq!(restored.is_fragment(), specified.is_fragment());
                assert_eq!(restored.to_css_string(), computed.to_css_string());
                let original = (specified.is_invalid() || specified.is_fragment()).then_some(text);
                assert_eq!(restored.original(), original);
            });
        }
    }

    #[test]
    fn canonical_storage_preserves_cors_modifiers_referrers_and_clones() {
        use crate::stylesheets::CorsMode;
        let url_data = test_url_data()
            .with_referrer_source(Some(url::Url::parse("https://cdn.test/style.css").unwrap()));
        let context = author_parser_context(&url_data);
        let css = "url(\"data:text/plain,canonical\" cross-origin(anonymous) integrity(\"sha256-proof\") referrer-policy(no-referrer))";
        let mut input = ParserInput::new(css);
        let specified = Parser::new(&mut input)
            .parse_entirely(|input| {
                CssUrl::parse_with_cors_mode(&context, input, CorsMode::Anonymous)
            })
            .unwrap();
        assert_eq!(specified.original(), Some("data:text/plain,canonical"));
        assert_eq!(specified.cors_mode(), CorsMode::Anonymous);
        assert_eq!(
            specified.referrer_source(),
            Some("https://cdn.test/style.css")
        );
        assert_eq!(specified.to_css_string(), css);
        assert!(servo_arc::Arc::ptr_eq(&specified.0, &specified.clone().0));
        with_computed_context(|context| {
            let computed = specified.to_computed_value(context);
            let restored = CssUrl::from_computed_value(&computed);
            assert_eq!(restored, specified);
            assert_eq!(restored.to_css_string(), css);
            assert_eq!(restored.cors_mode(), CorsMode::Anonymous);
            assert_eq!(restored.request_modifiers(), specified.request_modifiers());
            assert_eq!(restored.referrer_source(), specified.referrer_source());
        });
    }

    #[test]
    fn computed_urls_preserve_empty_and_fragment_only_references() {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        for (url, expected, invalid) in [("", "url(\"\")", true), ("#foo", "url(\"#foo\")", false)]
        {
            let specified =
                CssUrl::parse_from_string(url.into(), &context, crate::stylesheets::CorsMode::None);
            assert_eq!(specified.is_invalid(), invalid);
            with_computed_context(|context| {
                let computed = specified.to_computed_value(context);
                assert_eq!(computed.to_css_string(), expected);
                let restored = CssUrl::from_computed_value(&computed);
                assert_eq!(restored.to_css_string(), expected);
                assert_eq!(restored.is_fragment(), url.starts_with('#'));
            });
        }
    }

    #[test]
    fn computed_url_roundtrips_keep_the_response_referrer_separate_from_the_base() {
        let url_data = test_url_data().with_referrer_source(Some(
            url::Url::parse("https://cdn.test/response.css").unwrap(),
        ));
        let context = author_parser_context(&url_data);
        let specified = CssUrl::parse_from_string(
            "image.png".into(),
            &context,
            crate::stylesheets::CorsMode::None,
        );
        with_computed_context(|context| {
            let computed = specified.to_computed_value(context);
            assert_eq!(
                computed.referrer_source(),
                Some("https://cdn.test/response.css")
            );
            let restored = CssUrl::from_computed_value(&computed);
            assert_eq!(restored.referrer_source(), computed.referrer_source());
            assert_eq!(restored.as_str(), specified.as_str());
        });
    }

    #[test]
    fn specified_url_equality_retains_fetch_and_local_reference_semantics() {
        let url_data = test_url_data()
            .with_referrer_source(Some(url::Url::parse("https://cdn.test/one.css").unwrap()));
        let context = author_parser_context(&url_data);
        let parse = |css: &str| {
            let mut input = ParserInput::new(css);
            Parser::new(&mut input)
                .parse_entirely(|input| CssUrl::parse(&context, input))
                .unwrap()
        };
        let plain = parse("url('image.png')");
        assert_ne!(plain, parse("url('image.png' cross-origin(anonymous))"));
        assert_ne!(plain, parse("url('image.png' integrity('sha256-example'))"));
        assert_ne!(
            plain,
            parse("url('image.png' referrer-policy(no-referrer))")
        );
        let other_data = test_url_data()
            .with_referrer_source(Some(url::Url::parse("https://cdn.test/two.css").unwrap()));
        let other = CssUrl::parse_from_string(
            "image.png".into(),
            &author_parser_context(&other_data),
            crate::stylesheets::CorsMode::None,
        );
        assert_ne!(plain, other);
        let local = parse("url('#local')");
        assert_ne!(local, parse(&format!("url('{}')", local.as_str())));
    }

    fn parses<T: Parse>(css: &str, tainted: &[AttrTaintedRange]) -> bool {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data).with_attr_tainted(tainted);
        let mut input = ParserInput::new(css);
        Parser::new(&mut input)
            .parse_entirely(|input| T::parse(&context, input))
            .is_ok()
    }

    #[test]
    fn attr_tainted_urls_are_invalid() {
        let all = |css: &str| {
            [AttrTaintedRange {
                start: 0,
                end: css.len(),
            }]
        };
        for css in ["url(cat.png)", "url(\"cat.png\")"] {
            assert!(parses::<CssUrl>(css, &[]), "{css}");
            assert!(!parses::<CssUrl>(css, &all(css)), "{css}");
        }
        let css = "image-set(\"cat.png\" 1x)";
        assert!(parses::<Image>(css, &[]));
        assert!(!parses::<Image>(
            css,
            &[AttrTaintedRange { start: 10, end: 19 }]
        ));
        assert!(parses::<Image>(
            css,
            &[AttrTaintedRange { start: 20, end: 22 }]
        ));
    }
}
