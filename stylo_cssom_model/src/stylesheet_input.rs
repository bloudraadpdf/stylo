use encoding_rs::Encoding;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CssEncoding(&'static Encoding);

impl CssEncoding {
    #[must_use]
    pub fn new(label: &str) -> Option<Self> {
        Encoding::for_label(label.trim().as_bytes()).map(Self)
    }

    #[must_use]
    pub const fn encoding(self) -> &'static Encoding {
        self.0
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name()
    }
}

impl std::fmt::Debug for CssEncoding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("CssEncoding")
            .field(&self.name())
            .finish()
    }
}

impl std::hash::Hash for CssEncoding {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.name(), state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StylesheetEnvironmentEncoding(CssEncoding);

impl StylesheetEnvironmentEncoding {
    #[must_use]
    pub const fn new(encoding: &'static Encoding) -> Self {
        Self(CssEncoding(encoding))
    }

    #[must_use]
    pub const fn from_css_encoding(encoding: CssEncoding) -> Self {
        Self(encoding)
    }

    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        CssEncoding::new(label).map(Self::from_css_encoding)
    }

    #[must_use]
    pub const fn css_encoding(self) -> CssEncoding {
        self.0
    }

    #[must_use]
    pub const fn encoding(self) -> &'static Encoding {
        self.0.encoding()
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylesheetLinkEncoding {
    Declared(CssEncoding),
    Inherited,
}
