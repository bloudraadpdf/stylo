use url::Url;

/// The base URL used to resolve relative URLs in a CSS stylesheet.
///
/// A document retrieval URL cannot be passed where this type is required:
///
/// ```compile_fail
/// fn resolve_css_url(_: &stylo_cssom::CssStylesheetBaseUrl) {}
/// let retrieval_url = url::Url::parse("https://example.test/document.html").unwrap();
/// resolve_css_url(&retrieval_url);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssStylesheetBaseUrl(Url);

impl CssStylesheetBaseUrl {
    pub fn from_absolute(url: Url) -> Self {
        Self(url)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn join(&self, reference: &str) -> Result<Url, url::ParseError> {
        self.0.join(reference)
    }
}
