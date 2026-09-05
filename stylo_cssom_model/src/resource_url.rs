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
    url: String,
    modifiers: CssUrlRequestModifiers,
}

impl CssResourceUrl {
    pub fn new(url: impl Into<String>, modifiers: CssUrlRequestModifiers) -> Self {
        Self {
            url: url.into(),
            modifiers,
        }
    }

    pub fn without_modifiers(url: impl Into<String>) -> Self {
        Self::new(url, CssUrlRequestModifiers::default())
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }

    pub fn modifiers(&self) -> &CssUrlRequestModifiers {
        &self.modifiers
    }
}
