#![deny(unsafe_code)]

mod calibrated_color;
pub mod color_matrix;
pub use calibrated_color::{CalGrayParams, CalRgbParams, CalibratedColour, LabParams};

mod error;
mod resource_url;
pub use error::CssomStylesheetError;
pub use resource_url::{
    CssResourceUrl, CssUrlCorsMode, CssUrlReferrerPolicy, CssUrlRequestModifiers,
};

mod font_feature_values;
mod stylesheet_input;
pub use font_feature_values::{
    FontFeatureKind, FontFeatureMap, InvalidFontFeatureValueCount, RuleFontFeatureValues,
};
pub use stylesheet_input::{CssEncoding, StylesheetEnvironmentEncoding, StylesheetLinkEncoding};

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

/// Process-unique identity for the style state associated with one bound DOM
/// document.
///
/// Handles are allocated once when parser output is bound. They are never
/// reused, including when a DOM document is forked or cloned.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StyleDocumentHandle(u64);

impl StyleDocumentHandle {
    #[must_use]
    pub fn allocate() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let value = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("style document handle space exhausted");
        Self(value)
    }
}

macro_rules! non_reused_handle {
    ($name:ident, $counter:ident, $message:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name(u64);

        impl $name {
            fn allocate() -> Self {
                static $counter: AtomicU64 = AtomicU64::new(1);
                let value = $counter
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                        value.checked_add(1)
                    })
                    .expect($message);
                Self(value)
            }
        }
    };
}

non_reused_handle!(
    DeclarationHandle,
    NEXT_DECLARATION_HANDLE,
    "declaration handle space exhausted"
);
non_reused_handle!(
    StyleSlotHandle,
    NEXT_STYLE_SLOT_HANDLE,
    "style slot handle space exhausted"
);

mod stylesheet_graph;
pub use stylesheet_graph::{
    DetachedRuleLease, DetachedRuleListLease, DetachedStyleSheetLease, ImportBindingContext,
    ImportBindingHandle, ImportBindingLease, ImportBindingLoadState, InternalStylesheetRoot,
    PendingSubstitutionValue, PositionTryDescriptorName, PreparedRuleGraphUpdate,
    RuleBindingContext, RuleBlock, RuleConditionKind, RuleContainerCondition, RuleCssomData,
    RuleDeclaration, RuleDeclarationBlock, RuleDeclarationDomain, RuleGrammar, RuleGraphError,
    RuleGroupHeader, RuleHandle, RuleImportCorsMode, RuleImportLayer, RuleImportPrelude,
    RuleImportReferrerPolicy, RuleImportRequest, RuleKeyframeSelector, RuleLease, RuleListHandle,
    RuleListLease, RuleMutationRevision, RuleNamespaceContext, RuleNode, RuleSourceStamp,
    StyleOrigin, StyleShadowScopeHandle, StyleSheetAttachmentCandidate, StyleSheetAttachmentHandle,
    StyleSheetAttachmentLease, StyleSheetAttachmentOwner, StyleSheetCandidate,
    StyleSheetGraphCandidate, StyleSheetHandle, StyleSheetImportCandidate, StyleSheetLease,
    StyleSheetSourceContext, StyleSheetSourceKind, StyleTreeScopeHandle, TypedRulePayload,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CssWrapperIdentity {
    InlineDeclaration(DeclarationHandle),
    InlinePropertyMap(DeclarationHandle),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StandardPropertyId(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyKind {
    Longhand,
    Shorthand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Inheritedness {
    Inherited,
    Reset,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedOmNumericPropertyGrammar {
    Unsupported,
    NumberOnly,
    SvgUserUnitLength,
    PropertyGrammar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedOmSpecifiedNumericReification {
    Direct,
    Numeric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedOmPropertyMultiplicity {
    Single,
    CommaSeparated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedRetainedKeywordClass {
    DominantBaselineAlias,
    LogicalBorderNone,
    OverflowClipMarginPaddingBox,
    TextBoxTrimAlias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedStyleReificationClass {
    Direct,
    BackgroundSizeNumeric,
    BorderWidth,
    Color,
    Image,
    FontStretch,
    LetterSpacing,
    LineHeight,
    LogicalBorderRadius,
    RetainedKeyword(ComputedRetainedKeywordClass),
    TextDecorationSkip,
    Transform,
    ComputedRepresentation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComputedStyleValue {
    Associated(Arc<str>),
    Keyword(Arc<str>),
    Numeric(ComputedNumericValue),
    Image(ComputedImageValue),
    Transform(ComputedTransformValue),
    Unparsed(ComputedUnparsedValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedCssomValue {
    Associated(Arc<str>),
    Keyword(Arc<str>),
    CssPixel(f32),
}

impl ResolvedCssomValue {
    #[must_use]
    pub fn associated(value: impl Into<Arc<str>>) -> Self {
        Self::Associated(value.into())
    }

    #[must_use]
    pub fn keyword(value: impl Into<Arc<str>>) -> Self {
        Self::Keyword(value.into())
    }

    #[must_use]
    pub const fn css_pixel(value: f32) -> Self {
        Self::CssPixel(value)
    }

    #[must_use]
    pub fn serialized_segment(&self) -> Option<&str> {
        match self {
            Self::Associated(value) | Self::Keyword(value) => Some(value),
            Self::CssPixel(_) => None,
        }
    }

    #[must_use]
    pub fn to_css_string(&self) -> String {
        match self {
            Self::Associated(value) | Self::Keyword(value) => value.to_string(),
            Self::CssPixel(value) => {
                let rounded = value.round();
                let value = if (value - rounded).abs() < 1e-5 {
                    rounded
                } else {
                    *value
                };
                format!("{value}px")
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedNumericValue {
    pub value: f64,
    pub unit: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedImageValue {
    pub serialization: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedTransformValue {
    pub serialization: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedUnparsedValue {
    pub serialization: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputedValueProvenance {
    Keyword(Arc<str>),
    Numeric(Arc<str>),
    ZeroPercentage,
}

impl ComputedStyleValue {
    #[must_use]
    pub fn associated(value: impl Into<Arc<str>>) -> Self {
        Self::Associated(value.into())
    }

    #[must_use]
    pub fn keyword(value: impl Into<Arc<str>>) -> Self {
        Self::Keyword(value.into())
    }

    #[must_use]
    pub fn image(value: impl Into<Arc<str>>) -> Self {
        Self::Image(ComputedImageValue {
            serialization: value.into(),
        })
    }

    #[must_use]
    pub fn transform(value: impl Into<Arc<str>>) -> Self {
        Self::Transform(ComputedTransformValue {
            serialization: value.into(),
        })
    }

    #[must_use]
    pub fn unparsed(value: impl Into<Arc<str>>) -> Self {
        Self::Unparsed(ComputedUnparsedValue {
            serialization: value.into(),
        })
    }
}

impl ComputedRetainedKeywordClass {
    #[must_use]
    pub fn retained_keyword(self, keyword: &str) -> Option<&'static str> {
        match self {
            Self::DominantBaselineAlias if keyword.eq_ignore_ascii_case("text-top") => {
                Some("text-top")
            },
            Self::DominantBaselineAlias if keyword.eq_ignore_ascii_case("text-bottom") => {
                Some("text-bottom")
            },
            Self::LogicalBorderNone | Self::TextBoxTrimAlias
                if keyword.eq_ignore_ascii_case("none") =>
            {
                Some("none")
            },
            Self::OverflowClipMarginPaddingBox if keyword.eq_ignore_ascii_case("padding-box") => {
                Some("padding-box")
            },
            Self::TextBoxTrimAlias if keyword.eq_ignore_ascii_case("trim-start") => {
                Some("trim-start")
            },
            Self::TextBoxTrimAlias if keyword.eq_ignore_ascii_case("trim-end") => Some("trim-end"),
            Self::TextBoxTrimAlias if keyword.eq_ignore_ascii_case("trim-both") => {
                Some("trim-both")
            },
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterRoute {
    Unsupported,
    Stylo,
    ContainerShorthand,
    PositionTryShorthand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitialValue {
    pub serialized: &'static str,
    pub typed: Option<TypedInitialValue>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypedInitialValue {
    Opacity(Opacity),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertySchemaRow {
    pub id: StandardPropertyId,
    pub name: &'static str,
    pub kind: PropertyKind,
    pub inheritedness: Inheritedness,
    pub initial: InitialValue,
    pub shorthand_expansion: &'static [&'static str],
    pub parser: AdapterRoute,
    pub serializer: AdapterRoute,
    pub multiplicity: Option<TypedOmPropertyMultiplicity>,
    pub numeric_grammar: TypedOmNumericPropertyGrammar,
    pub specified_numeric_reification: Option<TypedOmSpecifiedNumericReification>,
    pub computed_reification: Option<ComputedStyleReificationClass>,
}

impl StandardPropertyId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub fn schema(self) -> &'static PropertySchemaRow {
        &STANDARD_PROPERTIES[self.index()]
    }
}

#[must_use]
pub fn property_schema(name: &str) -> Option<&'static PropertySchemaRow> {
    STANDARD_PROPERTIES.iter().find(|row| row.name == name)
}

#[must_use]
pub fn property_schema_at(index: usize) -> Option<&'static PropertySchemaRow> {
    STANDARD_PROPERTIES.get(index)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(f32);

impl Opacity {
    pub const ONE: Self = Self(1.0);

    #[must_use]
    pub fn new(value: f32) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn computed_value(self) -> f32 {
        self.0.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Importance {
    Normal,
    Important,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssWideKeyword {
    Initial,
    Inherit,
    Unset,
    Revert,
    RevertLayer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListSeparator {
    Space,
    Comma,
    Slash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UrlSourceContext {
    pub base_url: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpecifiedComponentValue {
    Ident(Arc<str>),
    AtKeyword(Arc<str>),
    Hash {
        value: Arc<str>,
        id: bool,
    },
    Number {
        value: f32,
        serialization: Arc<str>,
    },
    Percentage {
        value: f32,
        serialization: Arc<str>,
    },
    Dimension {
        value: f32,
        unit: Arc<str>,
        serialization: Arc<str>,
    },
    String(Arc<str>),
    Url {
        value: Arc<str>,
        source: UrlSourceContext,
    },
    Function {
        name: Arc<str>,
        arguments: Box<[Self]>,
    },
    Block {
        opening: char,
        values: Box<[Self]>,
    },
    Delimiter(char),
    Operator(Arc<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpecifiedStyleValue {
    CssWide(CssWideKeyword),
    Opacity(Opacity),
    TokenStream(Arc<str>),
    Components(Box<[SpecifiedComponentValue]>),
    List {
        separator: ListSeparator,
        values: Box<[SpecifiedStyleValue]>,
    },
}

impl SpecifiedComponentValue {
    fn recontextualize_urls(&mut self, base_url: &Arc<str>) {
        match self {
            Self::Url { source, .. } => source.base_url = base_url.clone(),
            Self::Function { arguments, .. } => {
                for argument in arguments {
                    argument.recontextualize_urls(base_url);
                }
            },
            Self::Block { values, .. } => {
                for value in values {
                    value.recontextualize_urls(base_url);
                }
            },
            Self::Ident(_)
            | Self::AtKeyword(_)
            | Self::Hash { .. }
            | Self::Number { .. }
            | Self::Percentage { .. }
            | Self::Dimension { .. }
            | Self::String(_)
            | Self::Delimiter(_)
            | Self::Operator(_) => {},
        }
    }
}

impl SpecifiedStyleValue {
    fn recontextualize_urls(&mut self, base_url: &Arc<str>) {
        match self {
            Self::Components(values) => {
                for value in values {
                    value.recontextualize_urls(base_url);
                }
            },
            Self::List { values, .. } => {
                for value in values {
                    value.recontextualize_urls(base_url);
                }
            },
            Self::CssWide(_) | Self::Opacity(_) | Self::TokenStream(_) => {},
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecifiedDeclaration {
    pub property: SpecifiedPropertyName,
    pub value: SpecifiedStyleValue,
    pub importance: Importance,
    pub shorthand_source: Option<SpecifiedShorthandSource>,
    pub shorthand_value: Option<SpecifiedStyleValue>,
    pub typed_om_representation: Option<TypedOmDeclaredValueRepresentation>,
}

impl SpecifiedDeclaration {
    fn recontextualize_urls(&mut self, base_url: &Arc<str>) {
        self.value.recontextualize_urls(base_url);
        if let Some(value) = &mut self.shorthand_value {
            value.recontextualize_urls(base_url);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecifiedShorthandSource {
    Parsed(StandardPropertyId),
    PendingSubstitution(StandardPropertyId),
    CssomMutation(StandardPropertyId),
    CssomMutationPendingSubstitution(StandardPropertyId),
}

impl SpecifiedShorthandSource {
    #[must_use]
    pub const fn property(self) -> StandardPropertyId {
        match self {
            Self::Parsed(property)
            | Self::PendingSubstitution(property)
            | Self::CssomMutation(property)
            | Self::CssomMutationPendingSubstitution(property) => property,
        }
    }

    #[must_use]
    pub const fn has_pending_substitution(self) -> bool {
        matches!(
            self,
            Self::PendingSubstitution(_) | Self::CssomMutationPendingSubstitution(_)
        )
    }

    #[must_use]
    pub const fn is_authored_ingress(self) -> bool {
        matches!(self, Self::Parsed(_) | Self::PendingSubstitution(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedOmDeclaredValueRepresentation {
    Keyword(Arc<str>),
    Numeric(Arc<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpecifiedPropertyName {
    Standard(StandardPropertyId),
    Custom(Arc<str>),
    Compatibility(InlineCompatibilityProperty),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineCompatibilityProperty {
    FlowTolerance,
    GridLanesPack,
    Continue,
    LegacyTextAlign,
    LineClamp,
    WebkitLineClamp,
    WebkitBoxDisplay,
}

impl InlineCompatibilityProperty {
    #[must_use]
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::FlowTolerance => "flow-tolerance",
            Self::GridLanesPack => "grid-lanes-pack",
            Self::Continue => "continue",
            Self::LegacyTextAlign => "text-align",
            Self::LineClamp => "line-clamp",
            Self::WebkitLineClamp => "-webkit-line-clamp",
            Self::WebkitBoxDisplay => "display",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecifiedValueConversionError {
    UnsupportedProperty,
    UnsupportedValue,
    UnresolvedOpacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StylePresentationProvenance {
    RawIngress,
    CanonicalMutation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineStyleContext {
    pub document: StyleDocumentHandle,
    pub base_url: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineDeclarationCandidate {
    declarations: Arc<[SpecifiedDeclaration]>,
    diagnostics: Arc<[InlineDeclarationDiagnostic]>,
    presentation: Option<Arc<str>>,
    provenance: StylePresentationProvenance,
    context: InlineStyleContext,
    hydrated: bool,
}

impl InlineDeclarationCandidate {
    fn recontextualize_urls(&mut self) {
        for declaration in Arc::make_mut(&mut self.declarations) {
            declaration.recontextualize_urls(&self.context.base_url);
        }
    }

    #[must_use]
    pub fn raw(
        context: InlineStyleContext,
        presentation: Option<impl Into<Arc<str>>>,
        declarations: impl Into<Arc<[SpecifiedDeclaration]>>,
    ) -> Self {
        Self {
            declarations: declarations.into(),
            diagnostics: Arc::from([]),
            presentation: presentation.map(Into::into),
            provenance: StylePresentationProvenance::RawIngress,
            context,
            hydrated: true,
        }
    }

    #[must_use]
    pub fn unparsed(
        context: InlineStyleContext,
        presentation: Option<impl Into<Arc<str>>>,
    ) -> Self {
        Self {
            declarations: Arc::from([]),
            diagnostics: Arc::from([]),
            presentation: presentation.map(Into::into),
            provenance: StylePresentationProvenance::RawIngress,
            context,
            hydrated: false,
        }
    }

    #[must_use]
    pub fn canonical(
        context: InlineStyleContext,
        presentation: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[SpecifiedDeclaration]>>,
    ) -> Self {
        Self {
            declarations: declarations.into(),
            diagnostics: Arc::from([]),
            presentation: Some(presentation.into()),
            provenance: StylePresentationProvenance::CanonicalMutation,
            context,
            hydrated: true,
        }
    }

    #[must_use]
    pub fn with_diagnostics(
        mut self,
        diagnostics: impl Into<Arc<[InlineDeclarationDiagnostic]>>,
    ) -> Self {
        self.diagnostics = diagnostics.into();
        self
    }

    fn validate(&self) -> Result<(), StyleTransactionError> {
        if self.provenance == StylePresentationProvenance::CanonicalMutation
            && self.presentation.is_none()
        {
            return Err(StyleTransactionError::InvalidCandidate);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineDeclarationDiagnostic {
    pub property: Arc<str>,
    pub declaration: Arc<str>,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleDomTarget {
    document: StyleDocumentHandle,
    slot: StyleSlotHandle,
    declaration: DeclarationHandle,
    declaration_revision: u64,
}

impl StyleDomTarget {
    #[must_use]
    pub const fn document(self) -> StyleDocumentHandle {
        self.document
    }

    #[must_use]
    pub const fn slot(self) -> StyleSlotHandle {
        self.slot
    }

    #[must_use]
    pub const fn declaration(self) -> DeclarationHandle {
        self.declaration
    }

    #[must_use]
    pub const fn declaration_revision(self) -> u64 {
        self.declaration_revision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleAttributeView {
    target: StyleDomTarget,
    text: Option<Arc<str>>,
    provenance: StylePresentationProvenance,
}

impl StyleAttributeView {
    #[must_use]
    pub const fn target(&self) -> StyleDomTarget {
        self.target
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub const fn provenance(&self) -> StylePresentationProvenance {
        self.provenance
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleDomInstall {
    view: StyleAttributeView,
}

impl StyleDomInstall {
    #[must_use]
    pub const fn target(&self) -> StyleDomTarget {
        self.view.target()
    }

    #[must_use]
    pub const fn view(&self) -> &StyleAttributeView {
        &self.view
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleTransactionError {
    WrongDocument,
    WrongSlot,
    WrongDeclaration,
    StaleRevision,
    InvalidCandidate,
    MissingDeclaration,
    DestinationCollision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImperativePropertyRegistrationInput {
    name: Arc<str>,
    syntax: Arc<str>,
    inherits: bool,
    initial_value: Option<Arc<str>>,
}

impl ImperativePropertyRegistrationInput {
    #[must_use]
    pub fn new(
        name: String,
        syntax: String,
        inherits: bool,
        initial_value: Option<String>,
    ) -> Option<Self> {
        (!name.is_empty() && !syntax.is_empty()).then(|| Self {
            name: name.into(),
            syntax: syntax.into(),
            inherits,
            initial_value: initial_value.map(Arc::from),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn syntax(&self) -> &str {
        &self.syntax
    }

    #[must_use]
    pub const fn inherits(&self) -> bool {
        self.inherits
    }

    #[must_use]
    pub fn initial_value(&self) -> Option<&str> {
        self.initial_value.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImperativePropertyRegistration {
    input: ImperativePropertyRegistrationInput,
    insertion_order: usize,
    revision: u64,
}

impl ImperativePropertyRegistration {
    #[must_use]
    pub fn name(&self) -> &str {
        self.input.name()
    }

    #[must_use]
    pub fn syntax(&self) -> &str {
        self.input.syntax()
    }

    #[must_use]
    pub const fn inherits(&self) -> bool {
        self.input.inherits()
    }

    #[must_use]
    pub fn initial_value(&self) -> Option<&str> {
        self.input.initial_value()
    }

    #[must_use]
    pub const fn insertion_order(&self) -> usize {
        self.insertion_order
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImperativePropertyRegistrationSnapshot {
    document: StyleDocumentHandle,
    revision: u64,
    registrations: Arc<[ImperativePropertyRegistration]>,
}

impl ImperativePropertyRegistrationSnapshot {
    #[must_use]
    pub const fn document(&self) -> StyleDocumentHandle {
        self.document
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn registrations(&self) -> impl ExactSizeIterator<Item = &ImperativePropertyRegistration> {
        self.registrations.iter()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ImperativePropertyRegistration> {
        self.registrations
            .iter()
            .find(|registration| registration.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImperativePropertyRegistrationError {
    DuplicateName,
}

#[derive(Debug)]
struct DeclarationRecord {
    document: StyleDocumentHandle,
    slot: StyleSlotHandle,
    handle: DeclarationHandle,
    revision: u64,
    declarations: Arc<[SpecifiedDeclaration]>,
    diagnostics: Arc<[InlineDeclarationDiagnostic]>,
    presentation: Option<Arc<str>>,
    provenance: StylePresentationProvenance,
    context: InlineStyleContext,
    hydrated: bool,
}

impl DeclarationRecord {
    fn target(&self) -> StyleDomTarget {
        StyleDomTarget {
            document: self.document,
            slot: self.slot,
            declaration: self.handle,
            declaration_revision: self.revision,
        }
    }

    fn install(&self) -> StyleDomInstall {
        StyleDomInstall {
            view: StyleAttributeView {
                target: self.target(),
                text: self.presentation.clone(),
                provenance: self.provenance,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeclarationLease {
    cell: Arc<Mutex<DeclarationRecord>>,
}

#[derive(Clone, Debug)]
pub struct InlineStyleProjection {
    handle: DeclarationHandle,
    revision: u64,
    context: InlineStyleContext,
    present: bool,
    declarations: Arc<[SpecifiedDeclaration]>,
    diagnostics: Arc<[InlineDeclarationDiagnostic]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineStyleFingerprint {
    handle: DeclarationHandle,
    revision: u64,
    context: InlineStyleContext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineStyleSemanticFingerprint {
    declarations: Arc<[SpecifiedDeclaration]>,
}

impl InlineStyleProjection {
    #[must_use]
    pub const fn handle(&self) -> DeclarationHandle {
        self.handle
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn context(&self) -> &InlineStyleContext {
        &self.context
    }

    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.present
    }

    #[must_use]
    pub fn declarations(&self) -> &[SpecifiedDeclaration] {
        &self.declarations
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[InlineDeclarationDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn fingerprint(&self) -> InlineStyleFingerprint {
        InlineStyleFingerprint {
            handle: self.handle,
            revision: self.revision,
            context: self.context.clone(),
        }
    }

    #[must_use]
    pub fn semantic_fingerprint(&self) -> InlineStyleSemanticFingerprint {
        InlineStyleSemanticFingerprint {
            declarations: self.declarations.clone(),
        }
    }
}

impl DeclarationLease {
    #[must_use]
    pub fn handle(&self) -> DeclarationHandle {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .handle
    }

    #[must_use]
    pub fn slot(&self) -> StyleSlotHandle {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .slot
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .revision
    }

    #[must_use]
    pub fn declarations(&self) -> Arc<[SpecifiedDeclaration]> {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .declarations
            .clone()
    }

    #[must_use]
    pub fn projection(&self) -> InlineStyleProjection {
        let record = self.cell.lock().expect("declaration cell mutex poisoned");
        InlineStyleProjection {
            handle: record.handle,
            revision: record.revision,
            context: record.context.clone(),
            present: record.presentation.is_some(),
            declarations: record.declarations.clone(),
            diagnostics: record.diagnostics.clone(),
        }
    }

    #[must_use]
    pub fn fingerprint(&self) -> InlineStyleFingerprint {
        self.projection().fingerprint()
    }

    #[must_use]
    pub fn semantic_fingerprint(&self) -> InlineStyleSemanticFingerprint {
        self.projection().semantic_fingerprint()
    }

    #[must_use]
    pub fn context(&self) -> InlineStyleContext {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .context
            .clone()
    }

    #[must_use]
    pub fn is_hydrated(&self) -> bool {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .hydrated
    }

    #[must_use]
    pub fn view(&self) -> StyleAttributeView {
        self.cell
            .lock()
            .expect("declaration cell mutex poisoned")
            .install()
            .view
    }
}

#[derive(Debug, PartialEq)]
pub struct PreparedStyleDomUpdate {
    document: StyleDocumentHandle,
    slot: StyleSlotHandle,
    declaration: DeclarationHandle,
    base_revision: u64,
    candidate: InlineDeclarationCandidate,
}

#[derive(Debug)]
pub struct StyleState {
    document: StyleDocumentHandle,
    declarations: HashMap<DeclarationHandle, Arc<Mutex<DeclarationRecord>>>,
    declaration_order: Vec<DeclarationHandle>,
    slots: HashMap<StyleSlotHandle, Weak<Mutex<DeclarationRecord>>>,
    imperative_registrations: Vec<ImperativePropertyRegistration>,
    imperative_registration_revision: u64,
    stylesheets: stylesheet_graph::StyleGraph,
}

impl StyleState {
    #[must_use]
    pub fn new(document: StyleDocumentHandle) -> Self {
        Self {
            document,
            declarations: HashMap::new(),
            declaration_order: Vec::new(),
            slots: HashMap::new(),
            imperative_registrations: Vec::new(),
            imperative_registration_revision: 0,
            stylesheets: stylesheet_graph::StyleGraph::default(),
        }
    }

    #[must_use]
    pub const fn document(&self) -> StyleDocumentHandle {
        self.document
    }

    pub fn declaration_handles(&self) -> impl ExactSizeIterator<Item = DeclarationHandle> + '_ {
        self.declaration_order.iter().copied()
    }

    #[must_use]
    pub fn declaration(&self, handle: DeclarationHandle) -> Option<DeclarationLease> {
        self.declarations.get(&handle).map(|cell| DeclarationLease {
            cell: Arc::clone(cell),
        })
    }

    #[must_use]
    pub fn declaration_for_slot(&self, slot: StyleSlotHandle) -> Option<DeclarationLease> {
        self.slots
            .get(&slot)
            .and_then(Weak::upgrade)
            .map(|cell| DeclarationLease { cell })
    }

    pub fn register_imperative_property(
        &mut self,
        input: ImperativePropertyRegistrationInput,
    ) -> Result<ImperativePropertyRegistrationSnapshot, ImperativePropertyRegistrationError> {
        if self
            .imperative_registrations
            .iter()
            .any(|registration| registration.name() == input.name())
        {
            return Err(ImperativePropertyRegistrationError::DuplicateName);
        }
        self.imperative_registration_revision = self
            .imperative_registration_revision
            .checked_add(1)
            .expect("imperative property registration revision space exhausted");
        self.imperative_registrations
            .push(ImperativePropertyRegistration {
                input,
                insertion_order: self.imperative_registrations.len(),
                revision: self.imperative_registration_revision,
            });
        Ok(self.imperative_property_registration_snapshot())
    }

    #[must_use]
    pub fn imperative_property_registration_snapshot(
        &self,
    ) -> ImperativePropertyRegistrationSnapshot {
        ImperativePropertyRegistrationSnapshot {
            document: self.document,
            revision: self.imperative_registration_revision,
            registrations: self.imperative_registrations.clone().into(),
        }
    }

    pub fn create_inline_attribute(
        &mut self,
        mut candidate: InlineDeclarationCandidate,
    ) -> Result<(DeclarationLease, StyleDomInstall), StyleTransactionError> {
        candidate.validate()?;
        if candidate.context.document != self.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        candidate.recontextualize_urls();
        let slot = StyleSlotHandle::allocate();
        let handle = DeclarationHandle::allocate();
        let record = Arc::new(Mutex::new(DeclarationRecord {
            document: self.document,
            slot,
            handle,
            revision: 0,
            declarations: candidate.declarations,
            diagnostics: candidate.diagnostics,
            presentation: candidate.presentation,
            provenance: candidate.provenance,
            context: candidate.context,
            hydrated: candidate.hydrated,
        }));
        let install = record
            .lock()
            .expect("declaration cell mutex poisoned")
            .install();
        self.slots.insert(slot, Arc::downgrade(&record));
        self.declarations.insert(handle, Arc::clone(&record));
        self.declaration_order.push(handle);
        Ok((DeclarationLease { cell: record }, install))
    }

    pub fn prepare_dom_update(
        &self,
        lease: &DeclarationLease,
        slot: StyleSlotHandle,
        candidate: InlineDeclarationCandidate,
    ) -> Result<PreparedStyleDomUpdate, StyleTransactionError> {
        candidate.validate()?;
        if candidate.context.document != self.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        let record = lease.cell.lock().expect("declaration cell mutex poisoned");
        if record.document != self.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        if record.slot != slot {
            return Err(StyleTransactionError::WrongSlot);
        }
        let stored = self
            .declarations
            .get(&record.handle)
            .ok_or(StyleTransactionError::MissingDeclaration)?;
        if !Arc::ptr_eq(stored, &lease.cell) {
            return Err(StyleTransactionError::WrongDeclaration);
        }
        Ok(PreparedStyleDomUpdate {
            document: self.document,
            slot,
            declaration: record.handle,
            base_revision: record.revision,
            candidate,
        })
    }

    pub fn commit_dom_update(
        &mut self,
        update: PreparedStyleDomUpdate,
    ) -> Result<StyleDomInstall, StyleTransactionError> {
        if update.document != self.document || update.candidate.context.document != self.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        update.candidate.validate()?;
        let cell = self
            .declarations
            .get(&update.declaration)
            .ok_or(StyleTransactionError::WrongDeclaration)?;
        let slot_cell = self
            .slots
            .get(&update.slot)
            .and_then(Weak::upgrade)
            .ok_or(StyleTransactionError::WrongSlot)?;
        if !Arc::ptr_eq(cell, &slot_cell) {
            return Err(StyleTransactionError::WrongSlot);
        }
        let mut record = cell.lock().expect("declaration cell mutex poisoned");
        if record.handle != update.declaration {
            return Err(StyleTransactionError::WrongDeclaration);
        }
        if record.slot != update.slot {
            return Err(StyleTransactionError::WrongSlot);
        }
        if record.revision != update.base_revision {
            return Err(StyleTransactionError::StaleRevision);
        }
        record.revision = record
            .revision
            .checked_add(1)
            .expect("declaration revision space exhausted");
        let mut candidate = update.candidate;
        candidate.recontextualize_urls();
        record.declarations = candidate.declarations;
        record.diagnostics = candidate.diagnostics;
        record.presentation = candidate.presentation;
        record.provenance = candidate.provenance;
        record.context = candidate.context;
        record.hydrated = candidate.hydrated;
        Ok(record.install())
    }

    pub fn copy_inline_attribute_to(
        &self,
        lease: &DeclarationLease,
        destination: &mut Self,
        context: InlineStyleContext,
    ) -> Result<(DeclarationLease, StyleDomInstall), StyleTransactionError> {
        if context.document != destination.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        let record = lease.cell.lock().expect("declaration cell mutex poisoned");
        let stored = self
            .declarations
            .get(&record.handle)
            .ok_or(StyleTransactionError::MissingDeclaration)?;
        if record.document != self.document || !Arc::ptr_eq(stored, &lease.cell) {
            return Err(StyleTransactionError::WrongDeclaration);
        }
        let candidate = InlineDeclarationCandidate {
            declarations: record.declarations.clone(),
            diagnostics: record.diagnostics.clone(),
            presentation: record.presentation.clone(),
            provenance: record.provenance,
            context,
            hydrated: record.hydrated,
        };
        destination.create_inline_attribute(candidate)
    }

    pub fn copy_inline_attribute(
        &mut self,
        lease: &DeclarationLease,
        context: InlineStyleContext,
    ) -> Result<(DeclarationLease, StyleDomInstall), StyleTransactionError> {
        if context.document != self.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        let candidate = {
            let record = lease.cell.lock().expect("declaration cell mutex poisoned");
            let stored = self
                .declarations
                .get(&record.handle)
                .ok_or(StyleTransactionError::MissingDeclaration)?;
            if record.document != self.document || !Arc::ptr_eq(stored, &lease.cell) {
                return Err(StyleTransactionError::WrongDeclaration);
            }
            InlineDeclarationCandidate {
                declarations: record.declarations.clone(),
                diagnostics: record.diagnostics.clone(),
                presentation: record.presentation.clone(),
                provenance: record.provenance,
                context,
                hydrated: record.hydrated,
            }
        };
        self.create_inline_attribute(candidate)
    }

    pub fn adopt_inline_attribute_to(
        &mut self,
        lease: &DeclarationLease,
        destination: &mut Self,
        context: InlineStyleContext,
    ) -> Result<StyleDomInstall, StyleTransactionError> {
        if context.document != destination.document {
            return Err(StyleTransactionError::WrongDocument);
        }
        let mut record = lease.cell.lock().expect("declaration cell mutex poisoned");
        let stored = self
            .declarations
            .get(&record.handle)
            .ok_or(StyleTransactionError::MissingDeclaration)?;
        if record.document != self.document || !Arc::ptr_eq(stored, &lease.cell) {
            return Err(StyleTransactionError::WrongDeclaration);
        }
        if destination.declarations.contains_key(&record.handle)
            || destination.slots.contains_key(&record.slot)
        {
            return Err(StyleTransactionError::DestinationCollision);
        }
        let handle = record.handle;
        let slot = record.slot;
        self.declarations.remove(&handle);
        self.declaration_order
            .retain(|candidate| *candidate != handle);
        self.slots.remove(&slot);
        record.document = destination.document;
        record.context = context;
        let base_url = record.context.base_url.clone();
        for declaration in Arc::make_mut(&mut record.declarations) {
            declaration.recontextualize_urls(&base_url);
        }
        record.revision = record
            .revision
            .checked_add(1)
            .expect("declaration revision space exhausted");
        let install = record.install();
        drop(record);
        destination
            .declarations
            .insert(handle, Arc::clone(&lease.cell));
        destination.declaration_order.push(handle);
        destination.slots.insert(slot, Arc::downgrade(&lease.cell));
        Ok(install)
    }

    pub fn recontextualize_inline_attributes(
        &mut self,
        base_url: Arc<str>,
    ) -> Vec<(DeclarationHandle, StyleDomInstall)> {
        let mut installs = Vec::new();
        for handle in &self.declaration_order {
            let cell = self
                .declarations
                .get(handle)
                .expect("declaration order must only contain live records");
            let mut record = cell.lock().expect("declaration cell mutex poisoned");
            if record.context.base_url == base_url {
                continue;
            }
            record.context = InlineStyleContext {
                document: self.document,
                base_url: base_url.clone(),
            };
            for declaration in Arc::make_mut(&mut record.declarations) {
                declaration.recontextualize_urls(&base_url);
            }
            record.revision = record
                .revision
                .checked_add(1)
                .expect("declaration revision space exhausted");
            installs.push((*handle, record.install()));
        }
        installs
    }

    pub fn fork(
        &self,
        document: StyleDocumentHandle,
    ) -> Result<
        (
            Self,
            Vec<(DeclarationHandle, DeclarationLease, StyleDomInstall)>,
            Vec<(StyleSheetHandle, StyleSheetLease)>,
        ),
        StyleTransactionError,
    > {
        let mut destination = Self::new(document);
        destination.imperative_registrations = self.imperative_registrations.clone();
        destination.imperative_registration_revision = self.imperative_registration_revision;
        let mut copies = Vec::with_capacity(self.declaration_order.len());
        for handle in &self.declaration_order {
            let cell = self
                .declarations
                .get(handle)
                .ok_or(StyleTransactionError::MissingDeclaration)?;
            let record = cell.lock().expect("declaration cell mutex poisoned");
            let candidate = InlineDeclarationCandidate {
                declarations: record.declarations.clone(),
                diagnostics: record.diagnostics.clone(),
                presentation: record.presentation.clone(),
                provenance: record.provenance,
                context: InlineStyleContext {
                    document,
                    base_url: record.context.base_url.clone(),
                },
                hydrated: record.hydrated,
            };
            drop(record);
            let (lease, install) = destination.create_inline_attribute(candidate)?;
            copies.push((*handle, lease, install));
        }
        let stylesheet_copies = self.fork_stylesheets_to(&mut destination);
        Ok((destination, copies, stylesheet_copies))
    }
}

include!("property_table.rs");

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        CssEncoding, CssWrapperIdentity, DeclarationHandle, ImperativePropertyRegistrationError,
        ImperativePropertyRegistrationInput, ImportBindingContext, ImportBindingLoadState,
        InlineDeclarationCandidate, InlineStyleContext, InternalStylesheetRoot, Opacity,
        PreparedStyleDomUpdate, ResolvedCssomValue, RuleBindingContext, RuleCssomData,
        RuleDeclaration, RuleDeclarationBlock, RuleDeclarationDomain, RuleGrammar, RuleGroupHeader,
        RuleImportLayer, RuleImportRequest, RuleNode, STANDARD_PROPERTIES, SpecifiedDeclaration,
        StyleDocumentHandle, StyleOrigin, StylePresentationProvenance,
        StyleSheetAttachmentCandidate, StyleSheetAttachmentOwner, StyleSheetCandidate,
        StyleSheetGraphCandidate, StyleSheetImportCandidate, StyleSheetSourceContext,
        StyleSheetSourceKind, StyleState, StyleTransactionError, StyleTreeScopeHandle,
        property_schema,
    };

    fn import_request(url: &str) -> RuleImportRequest {
        RuleImportRequest::new(
            url,
            RuleImportLayer::Absent,
            super::RuleImportPrelude::new(format!("url(\"{url}\")")),
        )
    }

    #[test]
    fn pending_declarations_validate_the_originating_shorthand() {
        let transition = property_schema("transition").unwrap().id;
        let duration = property_schema("transition-duration").unwrap().id;
        let valid = RuleDeclaration::from_pending_substitution(
            "transition-duration",
            transition,
            "var(--timing)",
            "https://example.test/styles/",
        )
        .unwrap();
        assert_eq!(valid.value(), "");
        let pending = valid.pending_substitution().unwrap();
        assert_eq!(pending.shorthand(), transition);
        assert_eq!(pending.tokens(), "var(--timing)");
        assert_eq!(pending.base_url(), "https://example.test/styles/");
        assert!(
            RuleDeclaration::from_pending_substitution(
                "color",
                transition,
                "var(--timing)",
                "about:blank"
            )
            .is_none()
        );
        assert!(
            RuleDeclaration::from_pending_substitution(
                "transition-duration",
                duration,
                "var(--timing)",
                "about:blank"
            )
            .is_none()
        );
    }

    fn context(document: StyleDocumentHandle, base: &str) -> InlineStyleContext {
        InlineStyleContext {
            document,
            base_url: Arc::from(base),
        }
    }

    fn raw(document: StyleDocumentHandle, text: Option<&str>) -> InlineDeclarationCandidate {
        InlineDeclarationCandidate::raw(
            context(document, "https://example.test/"),
            text,
            Arc::<[SpecifiedDeclaration]>::from([]),
        )
    }

    #[test]
    fn stylesheet_cells_own_stable_topology_revision_and_lifetime() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let source = StyleSheetSourceContext::inline(
            document,
            StyleOrigin::Author,
            Arc::from("https://example.test/base/"),
        );
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                source,
                [RuleNode::media(
                    "screen",
                    [RuleNode::style("a", "color: red")],
                )],
            ))
            .expect("the typed stylesheet must bind");
        let top = sheet.top_list();
        let media = top.rule(0).expect("the top list must own the media rule");
        let nested = media
            .nested_list()
            .expect("the media rule must own its nested list");
        let style = nested
            .rule(0)
            .expect("the nested list must own the style rule");

        let second_sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                Arc::<[RuleNode]>::from([]),
            ))
            .expect("the second stylesheet must bind");
        assert_ne!(sheet.handle(), second_sheet.handle());
        assert_ne!(media.handle(), style.handle());
        assert_eq!(top.parent_sheet(), sheet.handle());
        assert_eq!(media.parent_list(), Some(top.handle()));
        assert_eq!(nested.parent_rule(), Some(media.handle()));
        assert_eq!(style.parent_list(), Some(nested.handle()));
        assert_eq!(sheet.revision(), 0);

        let update = state
            .prepare_replace_stylesheet(&sheet, [RuleNode::style("b", "color: blue")])
            .expect("a current sheet must prepare replacement");
        state
            .commit_rule_graph_update(update)
            .expect("the current graph revision must commit");
        assert_eq!(sheet.revision(), 1);
        assert!(state.rule(media.handle()).is_none());
        assert_eq!(media.parent_list(), None);
        assert_eq!(style.parent_list(), Some(nested.handle()));
        assert_eq!(sheet.rule_path(media.handle()), None);
        assert_eq!(media.serialization(), "@media screen { a { color: red; } }");

        state
            .detach_stylesheet(sheet.handle())
            .expect("the live sheet must detach");
        assert!(state.stylesheet(sheet.handle()).is_none());
        assert_eq!(
            sheet
                .top_list()
                .rule(0)
                .expect("the lease keeps the replacement rule")
                .serialization(),
            "b { color: blue; }"
        );
    }

    #[test]
    fn internal_stylesheet_root_serialises_typed_rule_grammars() {
        let keyframes = RuleNode::keyframes(
            "fade",
            [
                RuleNode::keyframe("from", [RuleDeclaration::new("opacity", "0")]),
                RuleNode::keyframe("to", [RuleDeclaration::new("opacity", "1")]),
            ],
        )
        .expect("typed keyframes accept only keyframe children");
        let root = InternalStylesheetRoot::new(
            StyleOrigin::Author,
            [
                RuleNode::page("", [RuleDeclaration::new("margin", "2cm")]),
                RuleNode::font_face([
                    RuleDeclaration::new("font-family", "Fixture"),
                    RuleDeclaration::new("src", "local(Fixture)"),
                ]),
                RuleNode::counter_style("marks", [RuleDeclaration::new("symbols", r#""*""#)]),
                RuleNode::property("--amount", "<number>", false, Some("0")),
                keyframes,
                RuleNode::layer(
                    None::<Arc<str>>,
                    [RuleNode::internal_style(
                        "body",
                        [RuleDeclaration::new("margin", "0")],
                    )],
                ),
            ],
        );

        assert_eq!(
            root.projection_serialization(),
            concat!(
                "@page { margin: 2cm; }\n",
                "@font-face { font-family: Fixture; src: local(Fixture); }\n",
                "@counter-style marks { symbols: \"*\"; }\n",
                "@property --amount { syntax: \"<number>\"; inherits: false; initial-value: 0; }\n",
                "@keyframes fade { from { opacity: 0; } to { opacity: 1; } }\n",
                "@layer { body { margin: 0; } }",
            )
        );
    }

    #[test]
    fn stylesheet_attachment_cells_retain_context_and_never_reuse_identity() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the constructed stylesheet must bind");
        let candidate = StyleSheetAttachmentCandidate {
            tree_scope: StyleTreeScopeHandle::Document(document),
            adopter: document,
            environment_revision: 7,
            cascade_position: 2,
            owner: StyleSheetAttachmentOwner::ConstructedProjection,
            active: true,
            base_url: Some(Arc::from("https://example.test/base/")),
            encoding: Some(CssEncoding::new("utf-8").expect("UTF-8 is a supported encoding")),
        };
        let attachment = state
            .attach_stylesheet(&sheet, candidate.clone())
            .expect("the live stylesheet must accept an attachment");

        assert_eq!(attachment.sheet(), sheet.handle());
        assert_eq!(attachment.candidate(), candidate);
        assert_eq!(attachment.revision(), 0);
        assert!(
            state
                .stylesheet_attachment(attachment.handle())
                .is_some_and(|stored| stored.handle() == attachment.handle())
        );

        let mut updated = candidate.clone();
        updated.active = false;
        assert!(
            state
                .update_stylesheet_attachment(&attachment, updated.clone())
                .expect("the live attachment context must update")
        );
        assert_eq!(attachment.candidate(), updated);
        assert_eq!(attachment.revision(), 1);
        assert!(
            !state
                .update_stylesheet_attachment(&attachment, attachment.candidate())
                .expect("an unchanged attachment context must remain valid")
        );
        assert_eq!(attachment.revision(), 1);

        state
            .detach_stylesheet_attachment(&attachment)
            .expect("the live attachment must detach");
        assert!(state.stylesheet_attachment(attachment.handle()).is_none());
        assert_eq!(
            state.detach_stylesheet_attachment(&attachment),
            Err(super::RuleGraphError::WrongStylesheet)
        );

        let replacement = state
            .attach_stylesheet(&sheet, candidate)
            .expect("the stylesheet must accept a replacement attachment");
        assert_ne!(replacement.handle(), attachment.handle());
    }

    #[test]
    fn stylesheet_graph_rejects_stale_and_foreign_updates_atomically() {
        let document = StyleDocumentHandle::allocate();
        let other_document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let other = StyleState::new(other_document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the stylesheet must bind");
        let first = state
            .prepare_replace_stylesheet(&sheet, [RuleNode::style("b", "color: blue")])
            .expect("the first replacement must prepare");
        let stale = state
            .prepare_replace_stylesheet(&sheet, [RuleNode::style("c", "color: green")])
            .expect("the concurrent replacement must prepare");
        let original = sheet
            .top_list()
            .rule(0)
            .expect("the original rule")
            .handle();

        assert!(matches!(
            other.prepare_replace_stylesheet(&sheet, [RuleNode::style("x", "color: black")]),
            Err(super::RuleGraphError::WrongStylesheet)
        ));
        assert_eq!(sheet.revision(), 0);
        assert_eq!(
            sheet
                .top_list()
                .rule(0)
                .expect("the unchanged rule")
                .handle(),
            original
        );

        state
            .commit_rule_graph_update(first)
            .expect("the first update must commit");
        let committed = sheet
            .top_list()
            .rule(0)
            .expect("the committed rule")
            .handle();
        assert_ne!(committed, original);
        assert_eq!(
            state.commit_rule_graph_update(stale),
            Err(super::RuleGraphError::StaleRevision)
        );
        assert_eq!(sheet.revision(), 1);
        assert_eq!(
            sheet
                .top_list()
                .rule(0)
                .expect("the retained rule")
                .handle(),
            committed
        );

        state
            .detach_stylesheet(sheet.handle())
            .expect("the sheet must detach");
        let replacement = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                Arc::<[RuleNode]>::from([]),
            ))
            .expect("a new sheet must allocate");
        assert_ne!(replacement.handle(), sheet.handle());
    }

    #[test]
    fn rule_source_stamps_track_identity_and_only_actual_rule_mutations() {
        let mut state = StyleState::new(StyleDocumentHandle::allocate());
        let original = RuleNode::authored(
            RuleGrammar::PositionTry,
            "@position-try --side { left: 1px; }",
            [],
        );
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [original.clone()],
            ))
            .unwrap();
        let rules = sheet.top_list();
        let source = rules.rule(0).unwrap();
        let initial = source
            .snapshot()
            .payload()
            .source_stamp()
            .expect("a live rule snapshot retains its source");
        assert_eq!(initial.rule(), source.handle());
        assert_eq!(initial, source.snapshot().payload().source_stamp().unwrap());

        let update = state
            .prepare_insert_rule(&sheet, &rules, 0, RuleNode::style("other", "color: red"))
            .unwrap();
        state.commit_rule_graph_update(update).unwrap();
        assert_eq!(source.snapshot().payload().source_stamp(), Some(initial));
        let update = state
            .prepare_mutate_rule(&sheet, &rules, 1, original.clone())
            .unwrap();
        state.commit_rule_graph_update(update).unwrap();
        assert_eq!(source.snapshot().payload().source_stamp(), Some(initial));

        let update = state
            .prepare_mutate_rule(
                &sheet,
                &rules,
                1,
                RuleNode::authored(
                    RuleGrammar::PositionTry,
                    "@position-try --side { left: 2px; }",
                    [],
                ),
            )
            .unwrap();
        state.commit_rule_graph_update(update).unwrap();
        let changed = source.snapshot().payload().source_stamp().unwrap();
        assert_eq!(changed.rule(), initial.rule());
        assert_ne!(changed.revision(), initial.revision());

        let (_, _, copies) = state.fork(StyleDocumentHandle::allocate()).unwrap();
        let forked_sheet = copies
            .iter()
            .find_map(|(handle, sheet_copy)| (*handle == sheet.handle()).then_some(sheet_copy))
            .unwrap();
        let forked_rule = forked_sheet.top_list().rule(1).unwrap();
        assert_ne!(forked_rule.handle(), source.handle());
        assert_eq!(
            forked_rule.snapshot().payload().source_stamp(),
            Some(changed)
        );

        let update = state
            .prepare_replace_rule(&sheet, &rules, 1, original)
            .unwrap();
        state.commit_rule_graph_update(update).unwrap();
        let replacement = rules
            .rule(1)
            .unwrap()
            .snapshot()
            .payload()
            .source_stamp()
            .unwrap();
        assert_ne!(replacement.rule(), initial.rule());
        assert_eq!(source.snapshot().payload().source_stamp(), Some(changed));
    }

    #[test]
    fn stylesheet_rebinding_commits_rules_and_source_metadata_together() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://old.test/"),
                ),
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the stylesheet must bind");
        let replacement = StyleSheetCandidate::new(
            StyleSheetSourceContext {
                kind: StyleSheetSourceKind::Linked,
                origin: StyleOrigin::Author,
                document: Some(document),
                source_url: Some(Arc::from("https://new.test/sheet.css")),
                base_url: Some(Arc::from("https://new.test/")),
                encoding: Some(CssEncoding::new("utf-8").expect("UTF-8 is a supported encoding")),
            },
            [RuleNode::style("b", "color: blue")],
        )
        .with_media(Some("print"))
        .with_disabled(true);
        let update = state
            .prepare_rebind_stylesheet(&sheet, replacement)
            .expect("the complete replacement must prepare");

        assert_eq!(sheet.serialise(), "a { color: red; }");
        assert_eq!(
            sheet.source().base_url.as_deref(),
            Some("https://old.test/")
        );
        assert_eq!(sheet.media(), None);
        assert!(!sheet.disabled());

        state
            .commit_rule_graph_update(update)
            .expect("the complete replacement must commit");
        assert_eq!(sheet.serialise(), "b { color: blue; }");
        assert_eq!(
            sheet.source().source_url.as_deref(),
            Some("https://new.test/sheet.css")
        );
        assert_eq!(sheet.media().as_deref(), Some("print"));
        assert!(sheet.disabled());
        assert_eq!(sheet.revision(), 1);
    }

    #[test]
    fn inline_stylesheet_environment_rebinds_without_changing_linked_sources() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let encoding = CssEncoding::new("utf-8").expect("UTF-8 is a supported encoding");
        let inline = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://old.test/"),
                ),
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the inline sheet must bind");

        assert!(
            state
                .recontextualize_inline_stylesheet(
                    &inline,
                    Arc::from("https://new.test/"),
                    encoding
                )
                .expect("the inline environment must rebind")
        );
        assert_eq!(
            inline.source().base_url.as_deref(),
            Some("https://new.test/")
        );
        assert_eq!(inline.source().encoding, Some(encoding));
        assert_eq!(inline.revision(), 1);

        let linked = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext {
                    kind: StyleSheetSourceKind::Linked,
                    origin: StyleOrigin::Author,
                    document: Some(document),
                    source_url: Some(Arc::from("https://source.test/sheet.css")),
                    base_url: Some(Arc::from("https://source.test/sheet.css")),
                    encoding: Some(encoding),
                },
                [RuleNode::style("b", "color: blue")],
            ))
            .expect("the linked sheet must bind");
        assert!(
            !state
                .recontextualize_inline_stylesheet(
                    &linked,
                    Arc::from("https://ignored.test/"),
                    encoding
                )
                .expect("linked source context must remain fixed")
        );
        assert_eq!(
            linked.source().base_url.as_deref(),
            Some("https://source.test/sheet.css")
        );
        assert_eq!(linked.revision(), 0);
    }

    #[test]
    fn rule_blocks_retain_graph_identity_grammar_and_binding_context() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let rule =
            RuleNode::style("a", "color: red").with_declaration_block(RuleDeclarationBlock::new(
                RuleDeclarationDomain::Style,
                "color: red;",
                [RuleDeclaration::new("color", "red")],
            ));
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [rule],
            ))
            .expect("the constructed sheet must bind");
        let rule = sheet.top_list().rule(0).expect("the rule must exist");
        let block = rule.block().expect("the style rule must own a rule block");

        assert_eq!(block.sheet(), sheet.handle());
        assert_eq!(block.rule(), rule.handle());
        assert_eq!(block.grammar(), RuleDeclarationDomain::Style);
        assert_eq!(
            block.binding_context(),
            RuleBindingContext::AttachmentDependent
        );
        assert_eq!(block.declarations()[0].name(), "color");
        assert_eq!(block.declarations()[0].value(), "red");
    }

    #[test]
    fn stylesheet_rule_list_edits_allocate_and_detach_rule_cells() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the sheet must bind");
        let list = sheet.top_list();
        let original = list.rule(0).expect("the original rule");
        assert!(sheet.detached().is_none());
        assert!(list.detached().is_none());
        assert!(original.detached().is_none());

        let insert = state
            .prepare_insert_rule(&sheet, &list, 1, RuleNode::style("b", "color: blue"))
            .expect("the insertion must prepare");
        state
            .commit_rule_graph_update(insert)
            .expect("the insertion must commit");
        let inserted = list.rule(1).expect("the inserted rule");
        assert_ne!(inserted.handle(), original.handle());
        assert_eq!(sheet.rule_path(original.handle()), Some(vec![0]));
        assert_eq!(sheet.rule_path(inserted.handle()), Some(vec![1]));
        assert_eq!(sheet.revision(), 1);

        let delete = state
            .prepare_delete_rule(&sheet, &list, 0)
            .expect("the deletion must prepare");
        state
            .commit_rule_graph_update(delete)
            .expect("the deletion must commit");
        assert_eq!(sheet.revision(), 2);
        assert!(state.rule(original.handle()).is_none());
        assert_eq!(original.serialization(), "a { color: red; }");
        assert_eq!(sheet.rule_path(original.handle()), None);
        assert_eq!(sheet.rule_path(inserted.handle()), Some(vec![0]));
        let detached = original
            .detached()
            .expect("a removed rule permits detached mutation");
        assert!(matches!(
            detached.mutate(RuleNode::media("all", [])),
            Err(super::RuleGraphError::WrongRule)
        ));
        detached
            .mutate(RuleNode::style("c", "color: green"))
            .expect("detached rules remain mutable");
        assert_eq!(original.serialization(), "c { color: green; }");
        assert_eq!(sheet.serialise(), "b { color: blue; }");
        assert_eq!(
            list.rule(0).expect("the inserted rule remains").handle(),
            inserted.handle()
        );

        assert!(matches!(
            state.prepare_delete_rule(&sheet, &list, 1),
            Err(super::RuleGraphError::InvalidDeletionIndex { index: 1, len: 1 })
        ));
        assert_eq!(sheet.revision(), 2);
    }

    #[test]
    fn cssom_member_mutation_preserves_rule_and_nested_list_cells() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [RuleNode::authored_with_group_header(
                    RuleGrammar::Style,
                    "a { color: red; & > b { color: blue; } }",
                    [RuleNode::style("& > b", "color: blue")],
                    RuleGroupHeader::new("a"),
                )
                .with_declaration_block(RuleDeclarationBlock::new(
                    RuleDeclarationDomain::Style,
                    "color: red;",
                    [RuleDeclaration::new("color", "red")],
                ))
                .with_cssom_data(RuleCssomData::Style {
                    selector: Arc::from("a"),
                })
                .expect("style CSSOM data must match style grammar")],
            ))
            .expect("the stylesheet must bind");
        let list = sheet.top_list();
        let rule = list.rule(0).expect("the style rule must exist");
        let block = rule
            .block()
            .expect("the style rule must own its declaration cell");
        let nested = rule
            .nested_list()
            .expect("the style rule must own its nested list");
        let nested_rule = nested.rule(0).expect("the nested style rule must exist");
        let updated = RuleNode::authored(RuleGrammar::Style, "a { color: green; }", [])
            .with_declaration_block(RuleDeclarationBlock::new(
                RuleDeclarationDomain::Style,
                "color: green;",
                [RuleDeclaration::new("color", "green")],
            ))
            .with_cssom_data(RuleCssomData::Style {
                selector: Arc::from("a"),
            })
            .expect("style CSSOM data must match style grammar");

        let update = state
            .prepare_mutate_rule(&sheet, &list, 0, updated)
            .expect("the CSSOM member mutation must prepare");
        state
            .commit_rule_graph_update(update)
            .expect("the CSSOM member mutation must commit");

        let current = list.rule(0).expect("the style rule must remain");
        assert_eq!(current.handle(), rule.handle());
        assert_eq!(
            current
                .nested_list()
                .expect("the nested list must remain")
                .handle(),
            nested.handle()
        );
        assert_eq!(
            current
                .nested_list()
                .and_then(|list| list.rule(0))
                .expect("the nested rule must remain")
                .handle(),
            nested_rule.handle()
        );
        assert_eq!(
            current
                .block()
                .expect("the declaration block must remain")
                .declarations()[0]
                .value(),
            "green"
        );
        assert!(
            current
                .block()
                .expect("the declaration block must remain")
                .same_cell(&block)
        );
        assert_eq!(block.declarations()[0].value(), "green");
        assert_eq!(sheet.revision(), 1);
        assert!(matches!(
            state.prepare_mutate_rule(&sheet, &list, 0, RuleNode::media("screen", [])),
            Err(super::RuleGraphError::WrongRule)
        ));
        assert_eq!(sheet.revision(), 1);
    }

    #[test]
    fn empty_grouping_rule_owns_a_live_nested_list() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [RuleNode::media("screen", [])],
            ))
            .expect("the sheet must bind");
        let grouping = sheet
            .top_list()
            .rule(0)
            .expect("the grouping rule must exist");
        let nested = grouping
            .nested_list()
            .expect("an empty grouping rule must own its nested list");

        let insert = state
            .prepare_insert_rule(&sheet, &nested, 0, RuleNode::style("a", "color: red"))
            .expect("the empty nested list must accept an insertion");
        state
            .commit_rule_graph_update(insert)
            .expect("the nested insertion must commit");

        assert_eq!(nested.len(), 1);
        assert_eq!(sheet.serialise(), "@media screen { a { color: red; } }");
    }

    #[test]
    fn import_bindings_own_context_state_and_loaded_child_lifetime() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let import = RuleNode::authored(RuleGrammar::Import, "@import url(child.css);", [])
            .with_cssom_data(RuleCssomData::Import {
                request: import_request("child.css"),
            })
            .expect("import CSSOM data must match import grammar");
        let parent = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [import],
            ))
            .expect("the parent stylesheet must bind");
        let import_rule = parent
            .top_list()
            .rule(0)
            .expect("the import rule must exist");
        let child = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext {
                    kind: StyleSheetSourceKind::Imported,
                    origin: StyleOrigin::Author,
                    document: Some(document),
                    source_url: Some(Arc::from("https://example.test/child.css")),
                    base_url: Some(Arc::from("https://example.test/")),
                    encoding: None,
                },
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the imported stylesheet must bind");

        let binding = state
            .bind_import(
                &import_rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            )
            .expect("the import rule must accept its source binding");
        assert_eq!(binding.parent_sheet(), parent.handle());
        assert_eq!(binding.parent_rule(), import_rule.handle());
        assert_eq!(binding.context(), ImportBindingContext::Source);
        assert_eq!(
            binding.resolved_url().as_ref(),
            "https://example.test/child.css"
        );
        assert_eq!(binding.state(), ImportBindingLoadState::Pending);
        assert_eq!(binding.revision(), 0);
        assert!(matches!(
            state.bind_import(
                &import_rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            ),
            Err(super::RuleGraphError::ImportBindingAlreadyExists)
        ));

        state
            .complete_import(&binding, &child)
            .expect("the binding must own its loaded child");
        assert_eq!(
            binding.state(),
            ImportBindingLoadState::Loaded(child.handle())
        );
        assert_eq!(binding.revision(), 1);
        assert!(
            binding
                .loaded_child()
                .is_some_and(|loaded| loaded.same_cell(&child))
        );

        for media in ["print", ""] {
            let updated = import_rule
                .node()
                .with_cssom_media_condition(media)
                .expect("an import rule accepts a media-list mutation");
            let mutation = state
                .prepare_mutate_rule(&parent, &parent.top_list(), 0, updated)
                .expect("the import media mutation must prepare");
            state
                .commit_rule_graph_update(mutation)
                .expect("the media mutation must commit");
            assert_eq!(
                binding.state(),
                ImportBindingLoadState::Loaded(child.handle())
            );
            assert_eq!(binding.revision(), 1);
            assert!(
                binding
                    .loaded_child()
                    .is_some_and(|loaded| loaded.same_cell(&child))
            );
            assert_eq!(import_rule.import_bindings()[0].handle(), binding.handle());
        }

        let replacement = state
            .prepare_replace_rule(
                &parent,
                &parent.top_list(),
                0,
                RuleNode::style("b", "color: blue"),
            )
            .expect("the import replacement must prepare");
        state
            .commit_rule_graph_update(replacement)
            .expect("the import replacement must commit");
        assert!(state.import_binding(binding.handle()).is_none());
        assert!(state.stylesheet(child.handle()).is_none());
        assert_eq!(
            binding.state(),
            ImportBindingLoadState::Loaded(child.handle())
        );
        assert!(
            binding
                .loaded_child()
                .is_some_and(|loaded| loaded.same_cell(&child))
        );
        assert!(
            import_rule
                .import_bindings()
                .iter()
                .any(|retained| retained.same_cell(&binding))
        );
        assert!(matches!(
            state.complete_import(&binding, &child),
            Err(super::RuleGraphError::WrongImportBinding)
        ));

        let replacement_import =
            RuleNode::authored(RuleGrammar::Import, "@import url(child.css);", [])
                .with_cssom_data(RuleCssomData::Import {
                    request: import_request("child.css"),
                })
                .expect("import CSSOM data must match import grammar");
        let replacement = state
            .prepare_replace_rule(&parent, &parent.top_list(), 0, replacement_import)
            .expect("the replacement import must prepare");
        state
            .commit_rule_graph_update(replacement)
            .expect("the replacement import must commit");
        let replacement_rule = parent
            .top_list()
            .rule(0)
            .expect("the replacement import must exist");
        let replacement_binding = state
            .bind_import(
                &replacement_rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            )
            .expect("the replacement import must accept a fresh binding");
        assert_ne!(replacement_binding.handle(), binding.handle());
    }

    #[test]
    fn constructed_import_bindings_are_scoped_to_their_attachment() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let import_rule = || {
            RuleNode::authored(RuleGrammar::Import, "@import url(child.css);", [])
                .with_cssom_data(RuleCssomData::Import {
                    request: import_request("child.css"),
                })
                .expect("import CSSOM data must match import grammar")
        };
        let parent = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [import_rule()],
            ))
            .expect("the constructed stylesheet must bind");
        let other = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::constructed(StyleOrigin::Author),
                [import_rule()],
            ))
            .expect("the other constructed stylesheet must bind");
        let attachment = |cascade_position| StyleSheetAttachmentCandidate {
            tree_scope: StyleTreeScopeHandle::Document(document),
            adopter: document,
            environment_revision: 0,
            cascade_position,
            owner: StyleSheetAttachmentOwner::ConstructedProjection,
            active: true,
            base_url: Some(Arc::from("https://example.test/")),
            encoding: None,
        };
        let first = state
            .attach_stylesheet(&parent, attachment(0))
            .expect("the first projection must attach");
        let second = state
            .attach_stylesheet(&parent, attachment(1))
            .expect("the second projection must attach");
        let foreign = state
            .attach_stylesheet(&other, attachment(2))
            .expect("the foreign projection must attach");
        let rule = parent
            .top_list()
            .rule(0)
            .expect("the import rule must exist");

        assert!(matches!(
            state.bind_import(
                &rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            ),
            Err(super::RuleGraphError::WrongImportBinding)
        ));

        let first_binding = state
            .bind_import(
                &rule,
                ImportBindingContext::Attachment(first.handle()),
                "https://example.test/child.css",
            )
            .expect("the first attachment must have its own import binding");
        let second_binding = state
            .bind_import(
                &rule,
                ImportBindingContext::Attachment(second.handle()),
                "https://example.test/child.css",
            )
            .expect("the second attachment must have its own import binding");
        assert_ne!(first_binding.handle(), second_binding.handle());
        let imported = |selector, colour| {
            StyleSheetCandidate::new(
                StyleSheetSourceContext {
                    kind: StyleSheetSourceKind::Imported,
                    origin: StyleOrigin::Author,
                    document: Some(document),
                    source_url: Some(Arc::from("https://example.test/child.css")),
                    base_url: Some(Arc::from("https://example.test/")),
                    encoding: None,
                },
                [RuleNode::style(selector, format!("color: {colour}"))],
            )
        };
        let first_child = state
            .create_stylesheet(imported(".first", "red"))
            .expect("the first attachment child must bind");
        let second_child = state
            .create_stylesheet(imported(".second", "blue"))
            .expect("the second attachment child must bind");
        state
            .complete_import(&first_binding, &first_child)
            .expect("the first attachment child must complete");
        state
            .complete_import(&second_binding, &second_child)
            .expect("the second attachment child must complete");
        let first_projection =
            parent.serialise_projection(ImportBindingContext::Attachment(first.handle()));
        let second_projection =
            parent.serialise_projection(ImportBindingContext::Attachment(second.handle()));
        assert!(first_projection.contains(".first"));
        assert!(!first_projection.contains(".second"));
        assert!(second_projection.contains(".second"));
        assert!(!second_projection.contains(".first"));
        assert!(matches!(
            state.bind_import(
                &rule,
                ImportBindingContext::Attachment(first.handle()),
                "https://example.test/child.css",
            ),
            Err(super::RuleGraphError::ImportBindingAlreadyExists)
        ));
        assert!(matches!(
            state.bind_import(
                &rule,
                ImportBindingContext::Attachment(foreign.handle()),
                "https://example.test/child.css",
            ),
            Err(super::RuleGraphError::WrongImportBinding)
        ));

        let mut presentation_only = first.candidate();
        presentation_only.active = false;
        assert!(
            state
                .update_stylesheet_attachment(&first, presentation_only.clone())
                .expect("presentation-only attachment mutation must commit")
        );
        assert!(state.import_binding(first_binding.handle()).is_some());

        presentation_only.base_url = Some(Arc::from("https://example.test/rebased/"));
        assert!(
            state
                .update_stylesheet_attachment(&first, presentation_only)
                .expect("attachment environment mutation must commit")
        );
        assert!(state.import_binding(first_binding.handle()).is_none());
        assert!(state.stylesheet(first_child.handle()).is_none());
        assert!(state.import_binding(second_binding.handle()).is_some());

        state
            .detach_stylesheet_attachment(&second)
            .expect("the second projection must detach");
        assert!(state.import_binding(second_binding.handle()).is_none());
        assert!(state.stylesheet(second_child.handle()).is_none());
    }

    #[test]
    fn import_completion_replaces_only_its_previous_loaded_child() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let import = RuleNode::authored(RuleGrammar::Import, "@import url(child.css);", [])
            .with_cssom_data(RuleCssomData::Import {
                request: import_request("child.css"),
            })
            .expect("import CSSOM data must match import grammar");
        let parent = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [import],
            ))
            .expect("the parent stylesheet must bind");
        let child_candidate = || {
            StyleSheetCandidate::new(
                StyleSheetSourceContext {
                    kind: StyleSheetSourceKind::Imported,
                    origin: StyleOrigin::Author,
                    document: Some(document),
                    source_url: Some(Arc::from("https://example.test/child.css")),
                    base_url: Some(Arc::from("https://example.test/")),
                    encoding: None,
                },
                [RuleNode::style("a", "color: red")],
            )
        };
        let first = state
            .create_stylesheet(child_candidate())
            .expect("the first child must bind");
        let second = state
            .create_stylesheet(child_candidate())
            .expect("the replacement child must bind");
        let rule = parent
            .top_list()
            .rule(0)
            .expect("the import rule must exist");
        let binding = state
            .bind_import(
                &rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            )
            .expect("the source import must bind");

        state
            .complete_import(&binding, &first)
            .expect("the first child must complete");
        state
            .complete_import(&binding, &second)
            .expect("the validated replacement child must complete");
        assert!(state.stylesheet(first.handle()).is_none());
        assert!(state.stylesheet(second.handle()).is_some());
        assert_eq!(
            binding.state(),
            ImportBindingLoadState::Loaded(second.handle())
        );
        assert_eq!(binding.revision(), 2);

        state
            .fail_import(&binding)
            .expect("the current import must fail");
        assert!(state.stylesheet(second.handle()).is_none());
        assert_eq!(binding.state(), ImportBindingLoadState::Failed);
        assert!(binding.loaded_child().is_none());
        assert_eq!(binding.revision(), 3);
    }

    #[test]
    fn stylesheet_graph_candidates_publish_loaded_and_failed_import_bindings() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let import_rule = |url: &'static str| {
            RuleNode::authored(RuleGrammar::Import, format!("@import url({url});"), [])
                .with_cssom_data(RuleCssomData::Import {
                    request: import_request(url),
                })
                .expect("import CSSOM data must match import grammar")
        };
        let imported_source = |url: &'static str| StyleSheetSourceContext {
            kind: StyleSheetSourceKind::Imported,
            origin: StyleOrigin::Author,
            document: Some(document),
            source_url: Some(Arc::from(url)),
            base_url: Some(Arc::from(url)),
            encoding: None,
        };
        let loaded = StyleSheetGraphCandidate::new(
            StyleSheetCandidate::new(
                imported_source("https://example.test/loaded.css"),
                [RuleNode::style("p", "color: green")],
            ),
            [],
        );
        let root = StyleSheetGraphCandidate::new(
            StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [import_rule("loaded.css"), import_rule("missing.css")],
            ),
            [
                StyleSheetImportCandidate::loaded(0, "https://example.test/loaded.css", loaded),
                StyleSheetImportCandidate::failed(1, "https://example.test/missing.css"),
            ],
        );

        let root = state
            .create_stylesheet_graph(root)
            .expect("the validated stylesheet graph must publish atomically");
        let loaded = root
            .top_list()
            .rule(0)
            .expect("the loaded import must exist");
        let missing = root
            .top_list()
            .rule(1)
            .expect("the failed import must exist");

        assert_eq!(
            loaded.import_bindings()[0].context(),
            ImportBindingContext::Source
        );
        assert!(matches!(
            loaded.import_bindings()[0].state(),
            ImportBindingLoadState::Loaded(_)
        ));
        assert_eq!(
            missing.import_bindings()[0].state(),
            ImportBindingLoadState::Failed
        );
        assert_eq!(root.revision(), 4);
        assert_eq!(
            root.serialise_projection(ImportBindingContext::Source),
            "p { color: green; }"
        );
        let projected = root.projection_nodes(ImportBindingContext::Source);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].grammar(), RuleGrammar::Style);
        assert_eq!(
            projected[0].projection_serialization(),
            "p { color: green; }"
        );
        let child = loaded.import_bindings()[0]
            .loaded_child()
            .expect("the loaded import retains its child");
        state
            .set_stylesheet_disabled(&child, true)
            .expect("the child can be disabled");
        assert!(
            root.serialise_projection(ImportBindingContext::Source)
                .is_empty()
        );
        assert!(
            root.projection_nodes(ImportBindingContext::Source)
                .is_empty()
        );
        state
            .set_stylesheet_disabled(&child, false)
            .expect("the child can be enabled");
        assert_eq!(
            root.serialise_projection(ImportBindingContext::Source),
            "p { color: green; }"
        );
    }

    #[test]
    fn stylesheet_projection_keeps_authored_compatibility_text_out_of_cssom_text() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let selection = RuleNode::authored(RuleGrammar::Style, "::selection { color: green; }", []);
        let rule = RuleNode::authored_with_group_header(
            RuleGrammar::Container,
            "@container (width >= 400px) { ::selection { color: green; } }",
            [selection],
            RuleGroupHeader::new("@container (width >= 400px)"),
        )
        .with_projection_serialization(
            "@container (width >= 400px) { ::selection { color: green; } ::highlight(note) { color: green; } }",
        );
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    "https://example.test/".into(),
                ),
                [rule],
            ))
            .expect("the stylesheet must bind");

        assert!(!sheet.serialise().contains("::highlight(note)"));
        assert!(
            sheet
                .serialise_projection_source()
                .contains("::highlight(note)")
        );
        assert_eq!(
            sheet.serialise_projection(ImportBindingContext::Source),
            "@container (width >= 400px) { ::selection { color: green; } ::highlight(note) { color: green; } }"
        );
    }

    #[test]
    fn stylesheet_forks_preserve_import_topology_with_fresh_cells() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let import = RuleNode::authored(RuleGrammar::Import, "@import url(child.css);", [])
            .with_cssom_data(RuleCssomData::Import {
                request: import_request("child.css"),
            })
            .expect("import CSSOM data must match import grammar");
        let parent = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [import],
            ))
            .expect("the parent stylesheet must bind");
        let child = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext {
                    kind: StyleSheetSourceKind::Imported,
                    origin: StyleOrigin::Author,
                    document: Some(document),
                    source_url: Some(Arc::from("https://example.test/child.css")),
                    base_url: Some(Arc::from("https://example.test/")),
                    encoding: None,
                },
                [RuleNode::style("a", "color: red")],
            ))
            .expect("the imported stylesheet must bind");
        let source_rule = parent
            .top_list()
            .rule(0)
            .expect("the import rule must exist");
        let source_binding = state
            .bind_import(
                &source_rule,
                ImportBindingContext::Source,
                "https://example.test/child.css",
            )
            .expect("the source import must bind");
        state
            .complete_import(&source_binding, &child)
            .expect("the imported child must complete");

        let destination = StyleDocumentHandle::allocate();
        let (fork, _, copies) = state.fork(destination).expect("the style state must fork");
        let copied_parent = copies
            .iter()
            .find_map(|(source, copy)| (*source == parent.handle()).then_some(copy))
            .expect("the parent stylesheet must fork");
        let copied_child = copies
            .iter()
            .find_map(|(source, copy)| (*source == child.handle()).then_some(copy))
            .expect("the imported child must fork");
        let copied_rule = copied_parent
            .top_list()
            .rule(0)
            .expect("the copied import rule must exist");
        let copied_binding = copied_rule
            .import_bindings()
            .into_iter()
            .next()
            .expect("the copied import rule must retain its binding");

        assert_ne!(copied_parent.handle(), parent.handle());
        assert_ne!(copied_child.handle(), child.handle());
        assert_ne!(copied_rule.handle(), source_rule.handle());
        assert_ne!(copied_binding.handle(), source_binding.handle());
        assert_eq!(copied_binding.parent_sheet(), copied_parent.handle());
        assert_eq!(copied_binding.parent_rule(), copied_rule.handle());
        assert_eq!(
            copied_binding.state(),
            ImportBindingLoadState::Loaded(copied_child.handle())
        );
        assert!(
            copied_binding
                .loaded_child()
                .is_some_and(|loaded| loaded.same_cell(copied_child))
        );
        assert_eq!(copied_binding.revision(), source_binding.revision());
        assert!(fork.import_binding(copied_binding.handle()).is_some());
    }

    #[test]
    fn stylesheet_snapshots_and_forks_preserve_typed_rule_data() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let page = RuleNode::authored_with_group_header(
            RuleGrammar::Page,
            "@page named:left { margin-top: 10px; }",
            [],
            RuleGroupHeader::new("@page named:left"),
        )
        .with_declaration_block(RuleDeclarationBlock::new(
            RuleDeclarationDomain::Page,
            "margin-top: 10px;",
            [RuleDeclaration::new("margin-top", "10px")],
        ))
        .with_cssom_data(RuleCssomData::Page {
            selector: Arc::from("named:left"),
        })
        .expect("page CSSOM data must match page grammar");
        let sheet = state
            .create_stylesheet(StyleSheetCandidate::new(
                StyleSheetSourceContext::inline(
                    document,
                    StyleOrigin::Author,
                    Arc::from("https://example.test/"),
                ),
                [page],
            ))
            .expect("the stylesheet must bind");

        let snapshot = sheet
            .top_list()
            .rule(0)
            .expect("the page rule must exist")
            .snapshot();
        assert!(matches!(
            snapshot.cssom_data(),
            Some(RuleCssomData::Page { selector }) if selector.as_ref() == "named:left"
        ));
        assert_eq!(
            snapshot
                .payload()
                .declaration_block()
                .expect("the page block must remain typed")
                .serialization(),
            "margin-top: 10px;"
        );

        let destination = StyleDocumentHandle::allocate();
        let (_, _, copies) = state.fork(destination).expect("the style state must fork");
        let copied = &copies[0].1;
        let copied_page = copied
            .top_list()
            .rule(0)
            .expect("the copied page rule must exist")
            .snapshot();
        assert!(matches!(
            copied_page.cssom_data(),
            Some(RuleCssomData::Page { selector }) if selector.as_ref() == "named:left"
        ));
        assert_eq!(
            copied_page
                .payload()
                .declaration_block()
                .expect("the copied page block must remain typed")
                .serialization(),
            "margin-top: 10px;"
        );
    }

    #[test]
    fn imperative_registrations_are_ordered_revisioned_and_atomic() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let length = ImperativePropertyRegistrationInput::new(
            "--length".to_owned(),
            "<length>".to_owned(),
            false,
            Some("1px".to_owned()),
        )
        .expect("the validated registration input must be non-empty");
        let universal = ImperativePropertyRegistrationInput::new(
            "--tokens".to_owned(),
            "*".to_owned(),
            true,
            None,
        )
        .expect("the validated registration input must be non-empty");

        let first = state
            .register_imperative_property(length.clone())
            .expect("the first name must register");
        assert_eq!(first.document(), document);
        assert_eq!(first.revision(), 1);
        assert_eq!(first.registrations().count(), 1);
        let second = state
            .register_imperative_property(universal)
            .expect("the second name must register");
        assert_eq!(second.revision(), 2);
        assert_eq!(
            second
                .registrations()
                .map(|registration| (
                    registration.name(),
                    registration.insertion_order(),
                    registration.revision()
                ))
                .collect::<Vec<_>>(),
            [("--length", 0, 1), ("--tokens", 1, 2)]
        );

        assert_eq!(
            state.register_imperative_property(length),
            Err(ImperativePropertyRegistrationError::DuplicateName)
        );
        assert_eq!(state.imperative_property_registration_snapshot(), second);
    }

    #[test]
    fn schema_names_are_unique_and_opacity_is_typed() {
        for (index, row) in STANDARD_PROPERTIES.iter().enumerate() {
            assert_eq!(row.id.index(), index);
            assert_eq!(property_schema(row.name), Some(row));
            assert_eq!(
                STANDARD_PROPERTIES
                    .iter()
                    .filter(|candidate| candidate.name == row.name)
                    .count(),
                1
            );
        }
        assert_eq!(
            property_schema("opacity")
                .expect("opacity row")
                .initial
                .typed,
            Some(super::TypedInitialValue::Opacity(Opacity::ONE))
        );
        assert_eq!(Opacity::new(f32::NAN), None);
        let opacity = Opacity::new(2.0).expect("finite opacity");
        assert_eq!(opacity.value(), 2.0);
        assert_eq!(opacity.computed_value(), 1.0);
    }

    #[test]
    fn resolved_cssom_values_serialise_at_the_observable_boundary() {
        assert_eq!(ResolvedCssomValue::css_pixel(12.0).to_css_string(), "12px");
        assert_eq!(
            ResolvedCssomValue::css_pixel(20.700_000_762_939_453_f32).to_css_string(),
            "20.7px"
        );
        assert_eq!(ResolvedCssomValue::keyword("auto").to_css_string(), "auto");
        assert!(matches!(
            ResolvedCssomValue::associated("calc(10px + 5%)"),
            ResolvedCssomValue::Associated(value) if value.as_ref() == "calc(10px + 5%)"
        ));
    }

    #[test]
    fn declaration_dom_updates_validate_every_identity_before_writing() {
        let document = StyleDocumentHandle::allocate();
        let other_document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let (lease, initial) = state
            .create_inline_attribute(raw(document, Some("color: red")))
            .expect("the initial declaration must bind");
        let (other_lease, _) = state
            .create_inline_attribute(raw(document, None))
            .expect("the second declaration must bind");
        assert_eq!(
            state.declaration_handles().collect::<Vec<_>>(),
            vec![lease.handle(), other_lease.handle()]
        );
        let snapshot = || {
            (
                lease.revision(),
                lease.declarations(),
                lease.context(),
                lease.view(),
            )
        };
        let before = snapshot();

        assert_eq!(
            state.prepare_dom_update(
                &lease,
                lease.slot(),
                raw(other_document, Some("color: blue"))
            ),
            Err(StyleTransactionError::WrongDocument)
        );
        assert_eq!(snapshot(), before);
        assert_eq!(
            state.prepare_dom_update(
                &lease,
                other_lease.slot(),
                raw(document, Some("color: blue"))
            ),
            Err(StyleTransactionError::WrongSlot)
        );
        assert_eq!(snapshot(), before);

        let invalid = InlineDeclarationCandidate {
            declarations: Arc::<[SpecifiedDeclaration]>::from([]),
            diagnostics: Arc::from([]),
            presentation: None,
            provenance: StylePresentationProvenance::CanonicalMutation,
            context: context(document, "https://example.test/"),
            hydrated: true,
        };
        assert_eq!(
            state.prepare_dom_update(&lease, lease.slot(), invalid),
            Err(StyleTransactionError::InvalidCandidate)
        );
        assert_eq!(snapshot(), before);

        let wrong_handle = PreparedStyleDomUpdate {
            document,
            slot: lease.slot(),
            declaration: DeclarationHandle::allocate(),
            base_revision: lease.revision(),
            candidate: raw(document, Some("color: blue")),
        };
        assert_eq!(
            state.commit_dom_update(wrong_handle),
            Err(StyleTransactionError::WrongDeclaration)
        );
        assert_eq!(snapshot(), before);

        let wrong_document = PreparedStyleDomUpdate {
            document: other_document,
            slot: lease.slot(),
            declaration: lease.handle(),
            base_revision: lease.revision(),
            candidate: raw(document, Some("color: blue")),
        };
        assert_eq!(
            state.commit_dom_update(wrong_document),
            Err(StyleTransactionError::WrongDocument)
        );
        assert_eq!(snapshot(), before);

        let first = state
            .prepare_dom_update(&lease, lease.slot(), raw(document, Some("color: blue")))
            .expect("the first update must prepare");
        let replay = state
            .prepare_dom_update(&lease, lease.slot(), raw(document, Some("color: green")))
            .expect("the replay candidate must prepare at the same base revision");
        let installed = state
            .commit_dom_update(first)
            .expect("the matching update must commit");
        assert_eq!(
            installed.target().declaration_revision(),
            initial.target().declaration_revision() + 1
        );
        let after_success = snapshot();
        assert_eq!(installed.view().text(), Some("color: blue"));
        assert_eq!(
            state.commit_dom_update(replay),
            Err(StyleTransactionError::StaleRevision)
        );
        assert_eq!(snapshot(), after_success);
    }

    #[test]
    fn declaration_leases_keep_cells_stable_after_store_release() {
        let document = StyleDocumentHandle::allocate();
        let lease = {
            let mut state = StyleState::new(document);
            state
                .create_inline_attribute(raw(document, Some("width: 1px")))
                .expect("the declaration must bind")
                .0
        };
        assert_eq!(lease.view().text(), Some("width: 1px"));
        assert_eq!(lease.revision(), 0);
    }

    #[test]
    fn wrapper_identity_distinguishes_interfaces_for_one_declaration() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let (lease, _) = state
            .create_inline_attribute(raw(document, None))
            .expect("the declaration must bind");

        assert_ne!(
            CssWrapperIdentity::InlineDeclaration(lease.handle()),
            CssWrapperIdentity::InlinePropertyMap(lease.handle())
        );
    }

    #[test]
    fn copy_fork_and_adoption_obey_identity_and_context_rules() {
        let source_document = StyleDocumentHandle::allocate();
        let destination_document = StyleDocumentHandle::allocate();
        assert_ne!(source_document, destination_document);
        let mut source = StyleState::new(source_document);
        let (first, _) = source
            .create_inline_attribute(raw(source_document, Some("background: url(image.png)")))
            .expect("the first source declaration must bind");
        let (second, _) = source
            .create_inline_attribute(raw(source_document, None))
            .expect("the second source declaration must bind");

        let (fork, copies, _) = source
            .fork(destination_document)
            .expect("the store must fork");
        assert_eq!(copies.len(), 2);
        assert_eq!(fork.declaration_handles().count(), 2);
        assert_ne!(copies[0].1.handle(), first.handle());
        assert_ne!(copies[0].1.slot(), first.slot());
        assert_eq!(copies[0].1.view().text(), first.view().text());
        assert_eq!(copies[0].1.context().document, destination_document);
        assert_eq!(first.context().document, source_document);

        let local_copy = source
            .copy_inline_attribute(
                &first,
                context(source_document, "https://source.test/local-copy/"),
            )
            .expect("a same-store clone must copy");
        assert_ne!(local_copy.0.handle(), first.handle());
        assert_ne!(local_copy.0.slot(), first.slot());
        assert_eq!(local_copy.0.view().text(), first.view().text());
        assert_eq!(
            local_copy.0.context().base_url.as_ref(),
            "https://source.test/local-copy/"
        );

        let third_document = StyleDocumentHandle::allocate();
        let mut destination = StyleState::new(third_document);
        let copied = source
            .copy_inline_attribute_to(
                &first,
                &mut destination,
                context(third_document, "https://destination.test/copy/"),
            )
            .expect("an imported declaration must copy");
        assert_ne!(copied.0.handle(), first.handle());
        assert_ne!(copied.0.slot(), first.slot());
        assert_eq!(copied.0.view().text(), first.view().text());

        let source_revision = second.revision();
        let adopted = source
            .adopt_inline_attribute_to(
                &second,
                &mut destination,
                context(third_document, "https://destination.test/adopt/"),
            )
            .expect("a live declaration must transfer");
        assert_eq!(adopted.target().declaration(), second.handle());
        assert_eq!(adopted.target().slot(), second.slot());
        assert_eq!(second.context().document, third_document);
        assert_eq!(second.revision(), source_revision + 1);
        assert!(
            !source
                .declaration_handles()
                .any(|handle| handle == second.handle())
        );
        assert!(
            destination
                .declaration_handles()
                .any(|handle| handle == second.handle())
        );

        let copied_revision = copied.0.revision();
        let installs = destination
            .recontextualize_inline_attributes(Arc::from("https://destination.test/new-base/"));
        assert_eq!(installs.len(), 2);
        assert_eq!(copied.0.revision(), copied_revision + 1);
        assert_eq!(
            copied.0.context().base_url.as_ref(),
            "https://destination.test/new-base/"
        );
        assert_eq!(
            destination
                .recontextualize_inline_attributes(Arc::from("https://destination.test/new-base/")),
            Vec::new()
        );
    }

    #[test]
    fn absent_view_retains_its_cell_and_canonical_mutation_reinstates_it() {
        let document = StyleDocumentHandle::allocate();
        let mut state = StyleState::new(document);
        let (lease, _) = state
            .create_inline_attribute(raw(document, Some("color : red")))
            .expect("the raw declaration must bind");
        let initial_fingerprint = lease.fingerprint();
        let removed = state
            .prepare_dom_update(&lease, lease.slot(), raw(document, None))
            .and_then(|update| state.commit_dom_update(update))
            .expect("removal must commit");
        assert_eq!(removed.view().text(), None);
        assert_eq!(
            removed.view().provenance(),
            StylePresentationProvenance::RawIngress
        );
        let removed_projection = lease.projection();
        assert_eq!(removed_projection.handle(), lease.handle());
        assert_eq!(removed_projection.revision(), lease.revision());
        assert!(!removed_projection.is_present());
        assert_eq!(removed_projection.context(), &lease.context());
        assert_ne!(lease.fingerprint(), initial_fingerprint);
        let removed_fingerprint = lease.fingerprint();
        assert_eq!(
            state
                .declaration_for_slot(lease.slot())
                .expect("the removed declaration cell must remain")
                .handle(),
            lease.handle()
        );

        let reinstated = state
            .prepare_dom_update(
                &lease,
                lease.slot(),
                InlineDeclarationCandidate::canonical(
                    context(document, "https://example.test/"),
                    "color: blue;",
                    Arc::<[SpecifiedDeclaration]>::from([]),
                ),
            )
            .and_then(|update| state.commit_dom_update(update))
            .expect("retained declaration mutation must commit");
        assert_eq!(reinstated.view().text(), Some("color: blue;"));
        assert_eq!(
            reinstated.view().provenance(),
            StylePresentationProvenance::CanonicalMutation
        );
        assert_eq!(reinstated.target().declaration(), lease.handle());
        assert!(lease.projection().is_present());
        assert_ne!(lease.fingerprint(), removed_fingerprint);
    }
}
