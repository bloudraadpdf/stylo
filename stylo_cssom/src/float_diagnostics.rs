use std::cell::RefCell;

use cssparser::{Parser, ParserInput, SourceLocation};
use selectors::matching::QuirksMode;
use style::{
    error_reporting::{ContextualParseError, ParseErrorReporter},
    properties::declaration_block::parse_style_attribute,
    stylesheets::{CssRuleType, UrlExtraData},
};

use crate::context::ABOUT_BLANK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFloatDeclarationProperty {
    Float,

    FloatReference,

    FloatDefer,

    FloatOffset,

    FloatDeferPage,

    FloatDeferColumn,

    FloatSnapBlock,

    FloatSnapInline,

    SnapBlock,

    SnapInline,
}

impl PageFloatDeclarationProperty {
    #[must_use]
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::FloatReference => "float-reference",
            Self::FloatDefer => "float-defer",
            Self::FloatOffset => "float-offset",
            Self::FloatDeferPage => "float-defer-page",
            Self::FloatDeferColumn => "float-defer-column",
            Self::FloatSnapBlock => "float-snap-block",
            Self::FloatSnapInline => "float-snap-inline",
            Self::SnapBlock => "snap-block",
            Self::SnapInline => "snap-inline",
        }
    }

    pub fn from_css_name(name: &str) -> Option<Self> {
        match name {
            "float" => Some(Self::Float),
            "float-reference" => Some(Self::FloatReference),
            "float-defer" => Some(Self::FloatDefer),
            "float-offset" => Some(Self::FloatOffset),
            "float-defer-page" => Some(Self::FloatDeferPage),
            "float-defer-column" => Some(Self::FloatDeferColumn),
            "float-snap-block" => Some(Self::FloatSnapBlock),
            "float-snap-inline" => Some(Self::FloatSnapInline),
            "snap-block" => Some(Self::SnapBlock),
            "snap-inline" => Some(Self::SnapInline),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFloatDeclarationError {
    pub property: PageFloatDeclarationProperty,

    pub declaration: String,

    pub line: u32,

    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFloatDeclarationErrors {
    first: PageFloatDeclarationError,
    rest: Vec<PageFloatDeclarationError>,
}

impl PageFloatDeclarationErrors {
    pub fn from_vec(mut errors: Vec<PageFloatDeclarationError>) -> Result<(), Self> {
        if errors.is_empty() {
            return Ok(());
        }
        let first = errors.remove(0);
        Err(Self {
            first,
            rest: errors,
        })
    }

    #[must_use]
    pub const fn first(&self) -> &PageFloatDeclarationError {
        &self.first
    }

    pub fn iter(&self) -> impl Iterator<Item = &PageFloatDeclarationError> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        1 + self.rest.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }
}

impl std::fmt::Display for PageFloatDeclarationErrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} invalid page-float declaration(s); first is `{}` at {}:{}",
            self.len(),
            self.first.declaration,
            self.first.line,
            self.first.column,
        )
    }
}

impl std::error::Error for PageFloatDeclarationErrors {}

fn diagnostics(
    declarations: Vec<crate::source::SourceDeclaration<'_>>,
) -> Vec<PageFloatDeclarationError> {
    let reporter = FloatDeclarationErrorReporter::default();
    let url_data = UrlExtraData::from(ABOUT_BLANK.clone());
    for declaration in declarations {
        if PageFloatDeclarationProperty::from_css_name(&declaration.property).is_some() {
            reporter.audit_single_declaration(
                &format!("{}:{}", declaration.property, declaration.source_value),
                &url_data,
                SourceLocation {
                    line: declaration.line,
                    column: declaration.column,
                },
            );
        }
    }
    reporter.into_errors()
}

pub fn stylesheet_diagnostics(css: &str) -> Vec<PageFloatDeclarationError> {
    diagnostics(crate::source::stylesheet_declarations(css))
}

pub fn inline_declaration_diagnostics(
    css: &str,
) -> std::sync::Arc<[stylo_cssom_model::InlineDeclarationDiagnostic]> {
    diagnostics(crate::source::declarations(css))
        .into_iter()
        .map(|error| stylo_cssom_model::InlineDeclarationDiagnostic {
            property: error.property.css_name().into(),
            declaration: error.declaration.into(),
            line: error.line,
            column: error.column,
        })
        .collect::<Vec<_>>()
        .into()
}

#[derive(Default)]
struct FloatDeclarationErrorReporter {
    errors: RefCell<Vec<PageFloatDeclarationError>>,
    source_location_override: RefCell<Option<SourceLocation>>,
}

impl FloatDeclarationErrorReporter {
    fn into_errors(self) -> Vec<PageFloatDeclarationError> {
        self.errors.into_inner()
    }

    fn audit_single_declaration(
        &self,
        declaration: &str,
        url_data: &UrlExtraData,
        location: SourceLocation,
    ) {
        self.source_location_override.replace(Some(location));
        let _ = parse_style_attribute(
            declaration,
            url_data,
            Some(self),
            QuirksMode::NoQuirks,
            CssRuleType::Style,
        );
        self.source_location_override.replace(None);
    }
}

impl ParseErrorReporter for FloatDeclarationErrorReporter {
    fn report_error(
        &self,
        _url: &UrlExtraData,
        location: SourceLocation,
        error: ContextualParseError<'_>,
    ) {
        let ContextualParseError::UnsupportedPropertyDeclaration(declaration, _, _) = error else {
            return;
        };
        let Some(property) = declaration_property(declaration) else {
            return;
        };
        let location = self.source_location_override.borrow().unwrap_or(location);
        self.errors.borrow_mut().push(PageFloatDeclarationError {
            property,
            declaration: declaration.to_owned(),
            line: location.line,
            column: location.column,
        });
    }
}

fn declaration_property(declaration: &str) -> Option<PageFloatDeclarationProperty> {
    let mut input = ParserInput::new(declaration);
    let mut parser = Parser::new(&mut input);
    let property = parser.expect_ident_cloned().ok()?;
    parser.expect_colon().ok()?;
    PageFloatDeclarationProperty::from_css_name(property.as_ref())
}
