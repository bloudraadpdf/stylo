/// CORS credentials behavior selected by a CSS URL request modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CssUrlCorsMode {
    Anonymous,
    UseCredentials,
}

/// A referrer policy selected by a CSS URL request modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CssUrlReferrerPolicy {
    NoReferrer,
    NoReferrerWhenDowngrade,
    SameOrigin,
    Origin,
    StrictOrigin,
    OriginWhenCrossOrigin,
    StrictOriginWhenCrossOrigin,
    UnsafeUrl,
}

/// The canonical CSS Values 5 request-modifier slots for a resource URL.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CssUrlRequestModifiers {
    cors: Option<CssUrlCorsMode>,
    integrity: Option<String>,
    referrer_policy: Option<CssUrlReferrerPolicy>,
}

impl CssUrlRequestModifiers {
    pub fn new(
        cors: Option<CssUrlCorsMode>,
        integrity: Option<String>,
        referrer_policy: Option<CssUrlReferrerPolicy>,
    ) -> Self {
        Self {
            cors,
            integrity,
            referrer_policy,
        }
    }

    pub fn cors(&self) -> Option<CssUrlCorsMode> {
        self.cors
    }

    pub fn integrity(&self) -> Option<&str> {
        self.integrity.as_deref()
    }

    pub fn referrer_policy(&self) -> Option<CssUrlReferrerPolicy> {
        self.referrer_policy
    }

    pub fn is_empty(&self) -> bool {
        self.cors.is_none() && self.integrity.is_none() && self.referrer_policy.is_none()
    }
}

/// A resolved CSS resource URL whose request modifiers cannot be dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssResourceUrl {
    url: std::sync::Arc<str>,
    modifiers: CssUrlRequestModifiers,
    referrer_source: Option<String>,
}

impl CssResourceUrl {
    pub fn new(url: impl Into<String>, modifiers: CssUrlRequestModifiers) -> Self {
        Self {
            url: url.into().into(),
            modifiers,
            referrer_source: None,
        }
    }

    pub fn without_modifiers(url: impl Into<String>) -> Self {
        Self::new(url, CssUrlRequestModifiers::default())
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }

    pub fn with_referrer_source(mut self, source: Option<String>) -> Self {
        self.referrer_source = source;
        self
    }

    pub fn referrer_source(&self) -> Option<&str> {
        self.referrer_source.as_deref()
    }

    pub fn with_url(&self, url: impl Into<String>) -> Self {
        Self {
            url: url.into().into(),
            ..self.clone()
        }
    }

    pub fn modifiers(&self) -> &CssUrlRequestModifiers {
        &self.modifiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_url_text_and_replacements_preserve_request_context() {
        let source = CssResourceUrl::new(
            "https://example.test/image.svg",
            CssUrlRequestModifiers::new(
                Some(CssUrlCorsMode::UseCredentials),
                Some("sha256-example".into()),
                Some(CssUrlReferrerPolicy::SameOrigin),
            ),
        )
        .with_referrer_source(Some("https://example.test/document".into()));
        let cloned = source.clone();
        assert_eq!(source, cloned);
        assert_eq!(source.as_str().as_ptr(), cloned.as_str().as_ptr());
        let replaced = cloned.with_url("https://example.test/replacement.svg");
        assert_eq!(source.as_str(), "https://example.test/image.svg");
        assert_eq!(replaced.as_str(), "https://example.test/replacement.svg");
        assert_eq!(source.modifiers(), replaced.modifiers());
        assert_eq!(source.referrer_source(), replaced.referrer_source());
    }
}
