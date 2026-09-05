use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex, Weak},
};

use crate::{CssEncoding, StyleDocumentHandle, StyleState};

non_reused_handle!(
    StyleSheetHandle,
    NEXT_STYLE_SHEET_HANDLE,
    "stylesheet handle space exhausted"
);
non_reused_handle!(RuleHandle, NEXT_RULE_HANDLE, "rule handle space exhausted");
non_reused_handle!(
    ImportBindingHandle,
    NEXT_IMPORT_BINDING_HANDLE,
    "import binding handle space exhausted"
);
non_reused_handle!(
    RuleListHandle,
    NEXT_RULE_LIST_HANDLE,
    "rule-list handle space exhausted"
);
non_reused_handle!(
    StyleSheetAttachmentHandle,
    NEXT_STYLE_SHEET_ATTACHMENT_HANDLE,
    "stylesheet attachment handle space exhausted"
);

impl StyleSheetHandle {
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

macro_rules! cssom_handle_encoding {
    ($($handle:ident),+ $(,)?) => {$(
        impl $handle {
            #[must_use]
            pub const fn raw(self) -> u64 { self.0 }

            #[must_use]
            pub const fn from_raw(raw: u64) -> Option<Self> {
                if raw == 0 { None } else { Some(Self(raw)) }
            }
        }
    )+};
}

cssom_handle_encoding!(RuleHandle, RuleListHandle);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuleMutationRevision(u64);

impl RuleMutationRevision {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("rule mutation revision space exhausted"),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RuleSourceStamp {
    rule: RuleHandle,
    revision: RuleMutationRevision,
}

impl RuleSourceStamp {
    fn initial(rule: RuleHandle) -> Self {
        Self {
            rule,
            revision: RuleMutationRevision(0),
        }
    }

    #[must_use]
    pub const fn rule(self) -> RuleHandle {
        self.rule
    }

    #[must_use]
    pub const fn revision(self) -> RuleMutationRevision {
        self.revision
    }
}

impl StyleSheetAttachmentHandle {
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleOrigin {
    UserAgent,
    User,
    Author,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleSheetSourceKind {
    Inline,
    Linked,
    Imported,
    Constructed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StyleShadowScopeHandle(u64);

impl StyleShadowScopeHandle {
    #[must_use]
    pub fn allocate() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("style shadow-scope handle space exhausted"),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleTreeScopeHandle {
    Document(StyleDocumentHandle),
    Shadow(StyleShadowScopeHandle),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleSheetAttachmentOwner {
    InlineOwner,
    LinkedOwner,
    ProcessingInstruction,
    StaticCore,
    ConstructedProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheetAttachmentCandidate {
    pub tree_scope: StyleTreeScopeHandle,
    pub adopter: StyleDocumentHandle,
    pub environment_revision: u64,
    pub cascade_position: usize,
    pub owner: StyleSheetAttachmentOwner,
    pub active: bool,
    pub base_url: Option<Arc<str>>,
    pub encoding: Option<CssEncoding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheetSourceContext {
    pub kind: StyleSheetSourceKind,
    pub origin: StyleOrigin,
    pub document: Option<StyleDocumentHandle>,
    pub source_url: Option<Arc<str>>,
    pub base_url: Option<Arc<str>>,
    pub encoding: Option<CssEncoding>,
}

impl StyleSheetSourceContext {
    #[must_use]
    pub fn inline(document: StyleDocumentHandle, origin: StyleOrigin, base_url: Arc<str>) -> Self {
        Self {
            kind: StyleSheetSourceKind::Inline,
            origin,
            document: Some(document),
            source_url: None,
            base_url: Some(base_url),
            encoding: None,
        }
    }

    #[must_use]
    pub const fn constructed(origin: StyleOrigin) -> Self {
        Self {
            kind: StyleSheetSourceKind::Constructed,
            origin,
            document: None,
            source_url: None,
            base_url: None,
            encoding: None,
        }
    }

    #[must_use]
    pub fn constructed_in(
        document: StyleDocumentHandle,
        origin: StyleOrigin,
        base_url: Arc<str>,
    ) -> Self {
        Self {
            kind: StyleSheetSourceKind::Constructed,
            origin,
            document: Some(document),
            source_url: None,
            base_url: Some(base_url),
            encoding: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleGrammar {
    Style,
    Namespace,
    Import,
    Media,
    Supports,
    Container,
    FontFace,
    FontFeatureValues,
    FontPaletteValues,
    CounterStyle,
    Keyframes,
    Keyframe,
    Margin,
    Page,
    Property,
    LayerBlock,
    LayerStatement,
    Scope,
    StartingStyle,
    PositionTry,
    NestedDeclarations,
    ColorProfile,
    When,
    Else,
    Document,
    CustomMedia,
    Region,
    Footnote,
    Sidenote,
    BdColour,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleConditionKind {
    Media,
    Supports,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleImportLayer {
    Absent,
    Anonymous,
    Named(Arc<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleImportCorsMode {
    Anonymous,
    UseCredentials,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleImportReferrerPolicy {
    NoReferrer,
    NoReferrerWhenDowngrade,
    SameOrigin,
    Origin,
    StrictOrigin,
    OriginWhenCrossOrigin,
    StrictOriginWhenCrossOrigin,
    UnsafeUrl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleImportPrelude(Arc<str>);

impl RuleImportPrelude {
    #[must_use]
    pub fn new(serialization: impl Into<Arc<str>>) -> Self {
        Self(serialization.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleImportRequest {
    url: Arc<str>,
    prelude: RuleImportPrelude,
    cors: Option<RuleImportCorsMode>,
    integrity: Option<Arc<str>>,
    referrer_policy: Option<RuleImportReferrerPolicy>,
    layer: RuleImportLayer,
    supports: Option<Arc<str>>,
    media: Option<Arc<str>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportBindingContext {
    Source,
    Attachment(StyleSheetAttachmentHandle),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportBindingLoadState {
    Pending,
    Loaded(StyleSheetHandle),
    Failed,
}

impl RuleImportRequest {
    #[must_use]
    pub fn new(
        url: impl Into<Arc<str>>,
        layer: RuleImportLayer,
        prelude: RuleImportPrelude,
    ) -> Self {
        Self {
            url: url.into(),
            prelude,
            cors: None,
            integrity: None,
            referrer_policy: None,
            layer,
            supports: None,
            media: None,
        }
    }

    #[must_use]
    pub fn with_request_modifiers(
        mut self,
        cors: Option<RuleImportCorsMode>,
        integrity: Option<impl Into<Arc<str>>>,
        referrer_policy: Option<RuleImportReferrerPolicy>,
    ) -> Self {
        self.cors = cors;
        self.integrity = integrity.map(Into::into);
        self.referrer_policy = referrer_policy;
        self
    }

    #[must_use]
    pub fn with_conditions(
        mut self,
        supports: Option<impl Into<Arc<str>>>,
        media: Option<impl Into<Arc<str>>>,
    ) -> Self {
        self.supports = supports.map(Into::into);
        self.media = media.map(Into::into);
        self
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub const fn cors(&self) -> Option<RuleImportCorsMode> {
        self.cors
    }

    #[must_use]
    pub fn integrity(&self) -> Option<&str> {
        self.integrity.as_deref()
    }

    #[must_use]
    pub const fn referrer_policy(&self) -> Option<RuleImportReferrerPolicy> {
        self.referrer_policy
    }

    #[must_use]
    pub const fn layer(&self) -> &RuleImportLayer {
        &self.layer
    }

    #[must_use]
    pub fn supports(&self) -> Option<&str> {
        self.supports.as_deref()
    }

    #[must_use]
    pub fn media(&self) -> Option<&str> {
        self.media.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleContainerCondition {
    name: Arc<str>,
    query: Arc<str>,
}

impl RuleContainerCondition {
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>, query: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            query: query.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleCssomData {
    FontFeatureValues {
        values: crate::RuleFontFeatureValues,
    },
    Keyframes {
        name: Arc<str>,
    },
    Keyframe {
        selector: RuleKeyframeSelector,
    },
    Style {
        selector: Arc<str>,
    },
    Namespace {
        prefix: Arc<str>,
        uri: Arc<str>,
    },
    Import {
        request: RuleImportRequest,
    },
    Conditional {
        kind: RuleConditionKind,
        condition: Arc<str>,
    },
    Container {
        condition: Arc<str>,
        conditions: Arc<[RuleContainerCondition]>,
    },
    FontPaletteValues {
        name: Arc<str>,
        font_family: Arc<str>,
        base_palette: Arc<str>,
        override_colors: Arc<str>,
    },
    CounterStyle {
        name: Arc<str>,
        system: Arc<str>,
        negative: Arc<str>,
        prefix: Arc<str>,
        suffix: Arc<str>,
        range: Arc<str>,
        pad: Arc<str>,
        fallback: Arc<str>,
        symbols: Arc<str>,
        additive_symbols: Arc<str>,
    },
    Property {
        name: Arc<str>,
        syntax: Arc<str>,
        inherits: bool,
        initial_value: Option<Arc<str>>,
    },
    PositionTry {
        name: Arc<str>,
    },
    Margin {
        name: Arc<str>,
    },
    Page {
        selector: Arc<str>,
    },
    LayerBlock {
        name: Arc<str>,
    },
    LayerStatement {
        names: Arc<[Arc<str>]>,
    },
    Scope {
        start: Option<Arc<str>>,
        end: Option<Arc<str>>,
    },
}

impl RuleCssomData {
    const fn grammar(&self) -> RuleGrammar {
        match self {
            Self::Keyframes { .. } => RuleGrammar::Keyframes,
            Self::FontFeatureValues { .. } => RuleGrammar::FontFeatureValues,
            Self::Keyframe { .. } => RuleGrammar::Keyframe,
            Self::Style { .. } => RuleGrammar::Style,
            Self::Namespace { .. } => RuleGrammar::Namespace,
            Self::Import { .. } => RuleGrammar::Import,
            Self::Conditional {
                kind: RuleConditionKind::Media,
                ..
            } => RuleGrammar::Media,
            Self::Conditional {
                kind: RuleConditionKind::Supports,
                ..
            } => RuleGrammar::Supports,
            Self::Container { .. } => RuleGrammar::Container,
            Self::FontPaletteValues { .. } => RuleGrammar::FontPaletteValues,
            Self::CounterStyle { .. } => RuleGrammar::CounterStyle,
            Self::Property { .. } => RuleGrammar::Property,
            Self::PositionTry { .. } => RuleGrammar::PositionTry,
            Self::Margin { .. } => RuleGrammar::Margin,
            Self::Page { .. } => RuleGrammar::Page,
            Self::LayerBlock { .. } => RuleGrammar::LayerBlock,
            Self::LayerStatement { .. } => RuleGrammar::LayerStatement,
            Self::Scope { .. } => RuleGrammar::Scope,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuleKeyframeSelector(Arc<[f32]>);

impl Eq for RuleKeyframeSelector {}

impl RuleKeyframeSelector {
    #[must_use]
    pub fn new(percentages: impl Into<Arc<[f32]>>) -> Option<Self> {
        let percentages = percentages.into();
        (!percentages.is_empty() && percentages.iter().all(|value| (0.0..=1.0).contains(value)))
            .then_some(Self(percentages))
    }

    #[must_use]
    pub fn percentages(&self) -> &[f32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionTryDescriptorName(&'static str);

impl PositionTryDescriptorName {
    pub const ALL: &'static [&'static str] = &[
        "margin",
        "margin-top",
        "margin-right",
        "margin-bottom",
        "margin-left",
        "margin-block",
        "margin-block-start",
        "margin-block-end",
        "margin-inline",
        "margin-inline-start",
        "margin-inline-end",
        "inset",
        "top",
        "right",
        "bottom",
        "left",
        "inset-block",
        "inset-block-start",
        "inset-block-end",
        "inset-inline",
        "inset-inline-start",
        "inset-inline-end",
        "width",
        "min-width",
        "max-width",
        "height",
        "min-height",
        "max-height",
        "block-size",
        "min-block-size",
        "max-block-size",
        "inline-size",
        "min-inline-size",
        "max-inline-size",
        "place-self",
        "align-self",
        "justify-self",
        "position-anchor",
        "position-area",
    ];

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|candidate| *candidate == name)
            .map(Self)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleDeclarationDomain {
    Style,
    FontFaceDescriptor,
    Page,
    Margin,
    Keyframe,
    PositionTry,
    Nested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDeclaration {
    name: RuleDeclarationName,
    value: RuleDeclarationValue,
    important: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuleDeclarationName {
    Custom(Arc<str>),
    Predefined(Arc<str>),
}

impl RuleDeclarationName {
    fn new(name: Arc<str>) -> Self {
        if name.starts_with("--") {
            Self::Custom(name)
        } else {
            Self::Predefined(name)
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Custom(name) | Self::Predefined(name) => name,
        }
    }

    fn matches(&self, query: &str) -> bool {
        match self {
            Self::Custom(name) => name.as_ref() == query,
            Self::Predefined(name) => name.eq_ignore_ascii_case(query),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuleDeclarationValue {
    Serialized(Arc<str>),
    PendingSubstitution(PendingSubstitutionValue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSubstitutionValue {
    shorthand: crate::StandardPropertyId,
    tokens: Arc<str>,
    base_url: Arc<str>,
}

impl PendingSubstitutionValue {
    #[must_use]
    pub const fn shorthand(&self) -> crate::StandardPropertyId {
        self.shorthand
    }

    #[must_use]
    pub fn tokens(&self) -> &str {
        &self.tokens
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl RuleDeclaration {
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
        Self {
            name: RuleDeclarationName::new(name.into()),
            value: RuleDeclarationValue::Serialized(value.into()),
            important: false,
        }
    }

    #[must_use]
    pub fn from_pending_substitution(
        name: impl Into<Arc<str>>,
        shorthand: crate::StandardPropertyId,
        tokens: impl Into<Arc<str>>,
        base_url: impl Into<Arc<str>>,
    ) -> Option<Self> {
        let name = name.into();
        let schema = shorthand.schema();
        if schema.kind != crate::PropertyKind::Shorthand
            || !schema.shorthand_expansion.contains(&name.as_ref())
        {
            return None;
        }
        Some(Self {
            name: RuleDeclarationName::Predefined(name),
            value: RuleDeclarationValue::PendingSubstitution(PendingSubstitutionValue {
                shorthand,
                tokens: tokens.into(),
                base_url: base_url.into(),
            }),
            important: false,
        })
    }

    #[must_use]
    pub fn with_importance(mut self, important: bool) -> Self {
        self.important = important;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[must_use]
    pub fn matches_name(&self, query: &str) -> bool {
        self.name.matches(query)
    }

    #[must_use]
    pub fn value(&self) -> &str {
        match &self.value {
            RuleDeclarationValue::Serialized(value) => value,
            RuleDeclarationValue::PendingSubstitution(_) => "",
        }
    }

    #[must_use]
    pub const fn pending_substitution(&self) -> Option<&PendingSubstitutionValue> {
        match &self.value {
            RuleDeclarationValue::PendingSubstitution(value) => Some(value),
            RuleDeclarationValue::Serialized(_) => None,
        }
    }

    #[must_use]
    pub const fn important(&self) -> bool {
        self.important
    }

    fn serialization(&self) -> String {
        let importance = if self.important { " !important" } else { "" };
        format!("{}: {}{importance};", self.name(), self.value())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleNamespaceContext {
    default_namespace: Option<Arc<str>>,
    prefixes: Arc<BTreeMap<Arc<str>, Arc<str>>>,
}

impl RuleNamespaceContext {
    #[must_use]
    pub fn new(
        default_namespace: Option<Arc<str>>,
        prefixes: impl IntoIterator<Item = (Arc<str>, Arc<str>)>,
    ) -> Self {
        Self {
            default_namespace,
            prefixes: Arc::new(prefixes.into_iter().collect()),
        }
    }

    #[must_use]
    pub fn default_namespace(&self) -> Option<&str> {
        self.default_namespace.as_deref()
    }

    pub fn prefixes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.prefixes
            .iter()
            .map(|(prefix, namespace)| (prefix.as_ref(), namespace.as_ref()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDeclarationBlock {
    domain: RuleDeclarationDomain,
    namespaces: RuleNamespaceContext,
    serialization: Arc<str>,
    declarations: Arc<[RuleDeclaration]>,
    shorthand_values: Arc<[RuleDeclaration]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleBindingContext {
    SourceBound,
    AttachmentDependent,
}

#[derive(Clone, Debug)]
pub struct RuleBlock(Arc<Mutex<RuleBlockRecord>>);

#[derive(Debug)]
struct RuleBlockRecord {
    sheet: StyleSheetHandle,
    rule: RuleHandle,
    binding_context: RuleBindingContext,
    block: RuleDeclarationBlock,
}

impl RuleBlock {
    fn new(
        sheet: StyleSheetHandle,
        rule: RuleHandle,
        binding_context: RuleBindingContext,
        block: &RuleDeclarationBlock,
    ) -> Self {
        Self(Arc::new(Mutex::new(RuleBlockRecord {
            sheet,
            rule,
            binding_context,
            block: block.clone(),
        })))
    }

    fn replace(&self, binding_context: RuleBindingContext, block: &RuleDeclarationBlock) {
        let mut record = self.0.lock().expect("rule-block cell mutex poisoned");
        record.binding_context = binding_context;
        record.block = block.clone();
    }

    #[must_use]
    pub fn same_cell(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn sheet(&self) -> StyleSheetHandle {
        self.0.lock().expect("rule-block cell mutex poisoned").sheet
    }

    #[must_use]
    pub fn rule(&self) -> RuleHandle {
        self.0.lock().expect("rule-block cell mutex poisoned").rule
    }

    #[must_use]
    pub fn grammar(&self) -> RuleDeclarationDomain {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .domain
    }

    #[must_use]
    pub fn binding_context(&self) -> RuleBindingContext {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .binding_context
    }

    #[must_use]
    pub fn namespaces(&self) -> RuleNamespaceContext {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .namespaces
            .clone()
    }

    #[must_use]
    pub fn declarations(&self) -> Arc<[RuleDeclaration]> {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .declarations
            .clone()
    }

    #[must_use]
    pub fn shorthand_values(&self) -> Arc<[RuleDeclaration]> {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .shorthand_values
            .clone()
    }

    #[must_use]
    pub fn serialization(&self) -> Arc<str> {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .serialization
            .clone()
    }

    #[must_use]
    pub fn snapshot(&self) -> RuleDeclarationBlock {
        self.0
            .lock()
            .expect("rule-block cell mutex poisoned")
            .block
            .clone()
    }
}

impl RuleDeclarationBlock {
    #[must_use]
    pub fn new(
        domain: RuleDeclarationDomain,
        serialization: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        Self {
            domain,
            namespaces: RuleNamespaceContext::default(),
            serialization: serialization.into(),
            declarations: declarations.into(),
            shorthand_values: Arc::from([]),
        }
    }

    #[must_use]
    pub fn from_declarations(
        domain: RuleDeclarationDomain,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        let declarations = declarations.into();
        let serialization = serialise_declarations(&declarations);
        Self {
            domain,
            namespaces: RuleNamespaceContext::default(),
            serialization: serialization.into(),
            declarations,
            shorthand_values: Arc::from([]),
        }
    }

    #[must_use]
    pub fn with_shorthand_values(
        mut self,
        shorthand_values: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        self.shorthand_values = shorthand_values.into();
        self
    }

    #[must_use]
    pub fn with_namespaces(mut self, namespaces: RuleNamespaceContext) -> Self {
        self.namespaces = namespaces;
        self
    }

    #[must_use]
    pub const fn namespaces(&self) -> &RuleNamespaceContext {
        &self.namespaces
    }

    #[must_use]
    pub const fn domain(&self) -> RuleDeclarationDomain {
        self.domain
    }

    #[must_use]
    pub fn declarations(&self) -> &[RuleDeclaration] {
        &self.declarations
    }

    #[must_use]
    pub fn shorthand_values(&self) -> &[RuleDeclaration] {
        &self.shorthand_values
    }

    #[must_use]
    pub fn serialization(&self) -> &str {
        &self.serialization
    }
}

fn serialise_declarations(declarations: &[RuleDeclaration]) -> String {
    declarations
        .iter()
        .map(RuleDeclaration::serialization)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleGroupHeader(Arc<str>);

impl RuleGroupHeader {
    #[must_use]
    pub fn new(serialization: impl Into<Arc<str>>) -> Self {
        Self(serialization.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedRulePayload {
    grammar: RuleGrammar,
    prelude: Arc<str>,
    block: Option<Arc<str>>,
    nested: Arc<[RuleNode]>,
    authored: bool,
    projection_serialization: Option<Arc<str>>,
    group_header: Option<RuleGroupHeader>,
    declaration_block: Option<RuleDeclarationBlock>,
    cssom_data: Option<RuleCssomData>,
    source_stamp: Option<RuleSourceStamp>,
}

impl TypedRulePayload {
    fn new(
        grammar: RuleGrammar,
        prelude: impl Into<Arc<str>>,
        block: Option<impl Into<Arc<str>>>,
        nested: impl Into<Arc<[RuleNode]>>,
    ) -> Self {
        Self {
            grammar,
            prelude: prelude.into(),
            block: block.map(Into::into),
            nested: nested.into(),
            authored: false,
            projection_serialization: None,
            group_header: None,
            declaration_block: None,
            cssom_data: None,
            source_stamp: None,
        }
    }

    fn authored(grammar: RuleGrammar, serialization: Arc<str>, nested: Arc<[RuleNode]>) -> Self {
        Self {
            grammar,
            prelude: serialization,
            block: None,
            nested,
            authored: true,
            projection_serialization: None,
            group_header: None,
            declaration_block: None,
            cssom_data: None,
            source_stamp: None,
        }
    }

    fn authored_with_group_header(
        grammar: RuleGrammar,
        serialization: Arc<str>,
        nested: Arc<[RuleNode]>,
        group_header: RuleGroupHeader,
    ) -> Self {
        Self {
            grammar,
            prelude: serialization,
            block: None,
            nested,
            authored: true,
            projection_serialization: None,
            group_header: Some(group_header),
            declaration_block: None,
            cssom_data: None,
            source_stamp: None,
        }
    }

    #[must_use]
    pub const fn grammar(&self) -> RuleGrammar {
        self.grammar
    }

    #[must_use]
    pub const fn source_stamp(&self) -> Option<RuleSourceStamp> {
        self.source_stamp
    }

    #[must_use]
    pub const fn cssom_data(&self) -> Option<&RuleCssomData> {
        self.cssom_data.as_ref()
    }
    #[must_use]
    pub fn prelude(&self) -> &str {
        &self.prelude
    }
    #[must_use]
    pub fn block(&self) -> Option<&str> {
        self.block.as_deref()
    }
    #[must_use]
    pub fn nested(&self) -> &[RuleNode] {
        &self.nested
    }

    #[must_use]
    pub const fn declaration_block(&self) -> Option<&RuleDeclarationBlock> {
        self.declaration_block.as_ref()
    }
}

macro_rules! rule_nodes {
    ($($variant:ident => $grammar:ident),+ $(,)?) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum RuleNode { $($variant(TypedRulePayload)),+ }

        impl RuleNode {
            #[must_use]
            pub const fn payload(&self) -> &TypedRulePayload {
                match self { $(Self::$variant(payload) => payload),+ }
            }
            fn payload_mut(&mut self) -> &mut TypedRulePayload {
                match self { $(Self::$variant(payload) => payload),+ }
            }
            #[must_use]
            pub const fn grammar(&self) -> RuleGrammar { self.payload().grammar() }
        }
    };
}

rule_nodes! {
    Style => Style, Namespace => Namespace, Import => Import, Media => Media,
    Supports => Supports, Container => Container, FontFace => FontFace,
    FontFeatureValues => FontFeatureValues, FontPaletteValues => FontPaletteValues,
    CounterStyle => CounterStyle, Keyframes => Keyframes, Keyframe => Keyframe,
    Margin => Margin, Page => Page, Property => Property, LayerBlock => LayerBlock,
    LayerStatement => LayerStatement, Scope => Scope, StartingStyle => StartingStyle,
    PositionTry => PositionTry, NestedDeclarations => NestedDeclarations,
    ColorProfile => ColorProfile, When => When, Else => Else, Document => Document,
    CustomMedia => CustomMedia, Region => Region, Footnote => Footnote,
    Sidenote => Sidenote, BdColour => BdColour, Unknown => Unknown,
}

impl RuleNode {
    #[must_use]
    pub const fn accepts_nested_rules(&self) -> bool {
        matches!(
            self.grammar(),
            RuleGrammar::Style
                | RuleGrammar::Media
                | RuleGrammar::Supports
                | RuleGrammar::Container
                | RuleGrammar::Page
                | RuleGrammar::LayerBlock
                | RuleGrammar::Scope
                | RuleGrammar::StartingStyle
                | RuleGrammar::When
                | RuleGrammar::Else
                | RuleGrammar::Keyframes
        )
    }

    #[must_use]
    pub fn with_cssom_data(mut self, data: RuleCssomData) -> Option<Self> {
        (self.grammar() == data.grammar()).then(|| {
            self.payload_mut().cssom_data = Some(data);
            self
        })
    }

    #[must_use]
    pub const fn cssom_data(&self) -> Option<&RuleCssomData> {
        self.payload().cssom_data()
    }

    fn from_payload(grammar: RuleGrammar, payload: TypedRulePayload) -> Self {
        match grammar {
            RuleGrammar::Style => Self::Style(payload),
            RuleGrammar::Namespace => Self::Namespace(payload),
            RuleGrammar::Import => Self::Import(payload),
            RuleGrammar::Media => Self::Media(payload),
            RuleGrammar::Supports => Self::Supports(payload),
            RuleGrammar::Container => Self::Container(payload),
            RuleGrammar::FontFace => Self::FontFace(payload),
            RuleGrammar::FontFeatureValues => Self::FontFeatureValues(payload),
            RuleGrammar::FontPaletteValues => Self::FontPaletteValues(payload),
            RuleGrammar::CounterStyle => Self::CounterStyle(payload),
            RuleGrammar::Keyframes => Self::Keyframes(payload),
            RuleGrammar::Keyframe => Self::Keyframe(payload),
            RuleGrammar::Margin => Self::Margin(payload),
            RuleGrammar::Page => Self::Page(payload),
            RuleGrammar::Property => Self::Property(payload),
            RuleGrammar::LayerBlock => Self::LayerBlock(payload),
            RuleGrammar::LayerStatement => Self::LayerStatement(payload),
            RuleGrammar::Scope => Self::Scope(payload),
            RuleGrammar::StartingStyle => Self::StartingStyle(payload),
            RuleGrammar::PositionTry => Self::PositionTry(payload),
            RuleGrammar::NestedDeclarations => Self::NestedDeclarations(payload),
            RuleGrammar::ColorProfile => Self::ColorProfile(payload),
            RuleGrammar::When => Self::When(payload),
            RuleGrammar::Else => Self::Else(payload),
            RuleGrammar::Document => Self::Document(payload),
            RuleGrammar::CustomMedia => Self::CustomMedia(payload),
            RuleGrammar::Region => Self::Region(payload),
            RuleGrammar::Footnote => Self::Footnote(payload),
            RuleGrammar::Sidenote => Self::Sidenote(payload),
            RuleGrammar::BdColour => Self::BdColour(payload),
            RuleGrammar::Unknown => Self::Unknown(payload),
        }
    }

    #[must_use]
    pub fn with_declaration_block(mut self, declaration_block: RuleDeclarationBlock) -> Self {
        self.payload_mut().declaration_block = Some(declaration_block);
        self
    }

    #[must_use]
    pub fn with_projected_nested(
        mut self,
        nested: impl Into<Arc<[Self]>>,
        serialization: impl Into<Arc<str>>,
    ) -> Self {
        let payload = self.payload_mut();
        payload.nested = nested.into();
        payload.projection_serialization = Some(serialization.into());
        self
    }

    #[must_use]
    pub fn with_cssom_selector(mut self, selector: impl Into<Arc<str>>) -> Option<Self> {
        let retain_group_header = self.payload().group_header.is_some();
        let selector = selector.into();
        let cssom_data = match self.cssom_data()?.clone() {
            RuleCssomData::Style { .. } => RuleCssomData::Style {
                selector: selector.clone(),
            },
            RuleCssomData::Page { .. } => RuleCssomData::Page {
                selector: selector.clone(),
            },
            _ => return None,
        };
        let payload = self.payload_mut();
        payload.prelude = selector;
        payload.authored = false;
        payload.projection_serialization = None;
        payload.group_header = None;
        payload.cssom_data = Some(cssom_data);
        if retain_group_header {
            self.refresh_group_header();
        }
        Some(self)
    }

    #[must_use]
    pub fn with_cssom_media_condition(mut self, condition: impl Into<Arc<str>>) -> Option<Self> {
        let retain_group_header = self.payload().group_header.is_some();
        let condition = condition.into();
        let (prelude, cssom_data) = match self.cssom_data()? {
            RuleCssomData::Conditional {
                kind: RuleConditionKind::Media,
                ..
            } => (
                condition.clone(),
                RuleCssomData::Conditional {
                    kind: RuleConditionKind::Media,
                    condition: condition.clone(),
                },
            ),
            RuleCssomData::Import { request } => {
                let mut request = request.clone();
                request.media = (!condition.is_empty()).then_some(condition);
                let prelude = match request.media() {
                    Some(media) => Arc::from(format!("{} {media}", request.prelude.0)),
                    None => request.prelude.0.clone(),
                };
                (prelude, RuleCssomData::Import { request })
            },
            _ => return None,
        };
        let payload = self.payload_mut();
        payload.prelude = prelude;
        payload.authored = false;
        payload.projection_serialization = None;
        payload.group_header = None;
        payload.cssom_data = Some(cssom_data);
        if retain_group_header {
            self.refresh_group_header();
        }
        Some(self)
    }

    #[must_use]
    pub fn with_cssom_declaration_block(mut self, declaration_block: RuleDeclarationBlock) -> Self {
        let prelude =
            match self.cssom_data() {
                Some(RuleCssomData::Style { selector })
                | Some(RuleCssomData::Page { selector }) => Some(selector.clone()),
                Some(RuleCssomData::Margin { name })
                | Some(RuleCssomData::PositionTry { name }) => Some(name.clone()),
                _ if matches!(
                    self.grammar(),
                    RuleGrammar::FontFace | RuleGrammar::NestedDeclarations
                ) =>
                {
                    Some(Arc::from(""))
                },
                _ => None,
            };
        let payload = self.payload_mut();
        if let Some(prelude) = prelude {
            payload.prelude = prelude;
        }
        payload.block = Some(declaration_block.serialization.clone());
        payload.declaration_block = Some(declaration_block);
        payload.authored = false;
        payload.projection_serialization = None;
        self
    }

    fn refresh_group_header(&mut self) {
        let prelude = self.payload().prelude();
        let header = match self.grammar() {
            RuleGrammar::Style => prelude.to_owned(),
            RuleGrammar::Media => format!("@media {prelude}"),
            _ => format!(
                "@{}{}",
                grammar_name(self.grammar()),
                at_rule_prelude(prelude)
            ),
        };
        self.payload_mut().group_header = Some(RuleGroupHeader::new(header));
    }

    #[must_use]
    pub fn with_counter_style_data(mut self, data: RuleCssomData) -> Option<Self> {
        let RuleCssomData::CounterStyle {
            name,
            system,
            negative,
            prefix,
            suffix,
            range,
            pad,
            fallback,
            symbols,
            additive_symbols,
        } = &data
        else {
            return None;
        };
        if self.grammar() != RuleGrammar::CounterStyle {
            return None;
        }
        let mut descriptors = Vec::new();
        for (property, value) in [
            ("system", system),
            ("negative", negative),
            ("prefix", prefix),
            ("suffix", suffix),
            ("range", range),
            ("pad", pad),
            ("fallback", fallback),
            ("symbols", symbols),
            ("additive-symbols", additive_symbols),
        ] {
            if !value.is_empty() {
                descriptors.push(format!("{property}: {value};"));
            }
        }
        let payload = self.payload_mut();
        payload.prelude = name.clone();
        payload.block = Some(descriptors.join(" ").into());
        payload.authored = false;
        payload.projection_serialization = None;
        payload.group_header = None;
        payload.cssom_data = Some(data);
        Some(self)
    }

    #[must_use]
    pub fn with_authored_serialization(mut self, serialization: impl Into<Arc<str>>) -> Self {
        let payload = self.payload_mut();
        payload.prelude = serialization.into();
        payload.authored = true;
        self
    }

    #[must_use]
    pub fn with_projection_serialization(mut self, serialization: impl Into<Arc<str>>) -> Self {
        self.payload_mut().projection_serialization = Some(serialization.into());
        self
    }

    #[must_use]
    pub fn projection_serialization(&self) -> String {
        self.payload()
            .projection_serialization
            .as_deref()
            .map_or_else(|| self.serialization(), str::to_owned)
    }

    #[must_use]
    pub fn authored(
        grammar: RuleGrammar,
        serialization: impl Into<Arc<str>>,
        nested: impl Into<Arc<[Self]>>,
    ) -> Self {
        Self::from_payload(
            grammar,
            TypedRulePayload::authored(grammar, serialization.into(), nested.into()),
        )
    }

    #[must_use]
    pub fn authored_with_group_header(
        grammar: RuleGrammar,
        serialization: impl Into<Arc<str>>,
        nested: impl Into<Arc<[Self]>>,
        group_header: RuleGroupHeader,
    ) -> Self {
        Self::from_payload(
            grammar,
            TypedRulePayload::authored_with_group_header(
                grammar,
                serialization.into(),
                nested.into(),
                group_header,
            ),
        )
    }

    #[must_use]
    pub fn style(selector: impl Into<Arc<str>>, declarations: impl Into<Arc<str>>) -> Self {
        Self::Style(TypedRulePayload::new(
            RuleGrammar::Style,
            selector,
            Some(declarations),
            Arc::<[Self]>::from([]),
        ))
    }

    #[must_use]
    pub fn media(condition: impl Into<Arc<str>>, rules: impl Into<Arc<[Self]>>) -> Self {
        Self::Media(TypedRulePayload::new(
            RuleGrammar::Media,
            condition,
            None::<Arc<str>>,
            rules,
        ))
    }

    #[must_use]
    pub fn supports(condition: impl Into<Arc<str>>, rules: impl Into<Arc<[Self]>>) -> Self {
        Self::Supports(TypedRulePayload::new(
            RuleGrammar::Supports,
            condition,
            None::<Arc<str>>,
            rules,
        ))
    }

    #[must_use]
    pub fn layer(name: Option<impl Into<Arc<str>>>, rules: impl Into<Arc<[Self]>>) -> Self {
        Self::LayerBlock(TypedRulePayload::new(
            RuleGrammar::LayerBlock,
            name.map_or_else(|| Arc::from(""), Into::into),
            None::<Arc<str>>,
            rules,
        ))
    }

    #[must_use]
    pub fn internal_style(
        selector: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        Self::internal_declaration_rule(
            RuleGrammar::Style,
            selector,
            RuleDeclarationDomain::Style,
            declarations,
        )
    }

    #[must_use]
    pub fn keyframe(
        selector: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        Self::internal_declaration_rule(
            RuleGrammar::Keyframe,
            selector,
            RuleDeclarationDomain::Keyframe,
            declarations,
        )
    }

    #[must_use]
    pub fn keyframes(name: impl Into<Arc<str>>, frames: impl Into<Arc<[Self]>>) -> Option<Self> {
        let frames = frames.into();
        frames
            .iter()
            .all(|rule| rule.grammar() == RuleGrammar::Keyframe)
            .then(|| {
                Self::Keyframes(TypedRulePayload::new(
                    RuleGrammar::Keyframes,
                    name,
                    None::<Arc<str>>,
                    frames,
                ))
            })
    }

    #[must_use]
    pub fn font_face(declarations: impl Into<Arc<[RuleDeclaration]>>) -> Self {
        Self::internal_declaration_rule(
            RuleGrammar::FontFace,
            "",
            RuleDeclarationDomain::FontFaceDescriptor,
            declarations,
        )
    }

    #[must_use]
    pub fn nested_declarations(declarations: impl Into<Arc<[RuleDeclaration]>>) -> Self {
        Self::internal_declaration_rule(
            RuleGrammar::NestedDeclarations,
            "",
            RuleDeclarationDomain::Nested,
            declarations,
        )
    }

    #[must_use]
    pub fn property(
        name: impl Into<Arc<str>>,
        syntax: impl Into<Arc<str>>,
        inherits: bool,
        initial_value: Option<impl Into<Arc<str>>>,
    ) -> Self {
        let name = name.into();
        let syntax = syntax.into();
        let initial_value = initial_value.map(Into::into);
        let escaped_syntax = syntax.replace('\\', "\\\\").replace('"', "\\\"");
        let mut block = format!("syntax: \"{escaped_syntax}\"; inherits: {inherits};");
        if let Some(initial_value) = &initial_value {
            block.push_str(" initial-value: ");
            block.push_str(initial_value);
            block.push(';');
        }
        Self::Property(TypedRulePayload::new(
            RuleGrammar::Property,
            name.clone(),
            Some(Arc::<str>::from(block)),
            Arc::<[Self]>::from([]),
        ))
        .with_cssom_data(RuleCssomData::Property {
            name,
            syntax,
            inherits,
            initial_value,
        })
        .expect("property CSSOM data matches the property grammar")
    }

    #[must_use]
    pub fn page(
        selector: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        let selector = selector.into();
        Self::internal_declaration_rule(
            RuleGrammar::Page,
            selector.clone(),
            RuleDeclarationDomain::Page,
            declarations,
        )
        .with_cssom_data(RuleCssomData::Page { selector })
        .expect("page CSSOM data matches the page grammar")
    }

    fn internal_declaration_rule(
        grammar: RuleGrammar,
        prelude: impl Into<Arc<str>>,
        domain: RuleDeclarationDomain,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        let block = RuleDeclarationBlock::from_declarations(domain, declarations);
        Self::from_payload(
            grammar,
            TypedRulePayload::new(
                grammar,
                prelude,
                Some(block.serialization.clone()),
                Arc::<[Self]>::from([]),
            ),
        )
        .with_declaration_block(block)
    }

    #[must_use]
    pub fn counter_style(
        name: impl Into<Arc<str>>,
        declarations: impl Into<Arc<[RuleDeclaration]>>,
    ) -> Self {
        let declarations = declarations.into();
        Self::CounterStyle(TypedRulePayload::new(
            RuleGrammar::CounterStyle,
            name,
            Some(Arc::<str>::from(serialise_declarations(&declarations))),
            Arc::<[Self]>::from([]),
        ))
    }

    pub fn serialization(&self) -> String {
        let payload = self.payload();
        if payload.authored {
            return payload.prelude().to_owned();
        }
        match self {
            Self::Style(_) => {
                serialise_qualified_rule(payload.prelude(), &serialise_rule_body(payload))
            },
            Self::Keyframe(_) => format!(
                "{} {{ {} }}",
                payload.prelude(),
                canonical_declaration_block(payload.block().unwrap_or_default())
            ),
            Self::Media(_) => format!(
                "@media {} {{ {} }}",
                payload.prelude(),
                serialise_rules(payload.nested())
            ),
            Self::Page(_) => {
                let prelude = at_rule_prelude(payload.prelude());
                serialise_qualified_rule(&format!("@page{prelude}"), &serialise_rule_body(payload))
            },
            Self::NestedDeclarations(_) => {
                canonical_declaration_block(payload.block().unwrap_or_default())
            },
            _ if let Some(block) = payload.block() => {
                let prelude = at_rule_prelude(payload.prelude());
                format!(
                    "@{}{prelude} {{ {block} }}",
                    grammar_name(payload.grammar())
                )
            },
            _ if payload.nested().is_empty() => {
                let prelude = at_rule_prelude(payload.prelude());
                format!("@{}{prelude};", grammar_name(payload.grammar()))
            },
            _ => {
                let prelude = at_rule_prelude(payload.prelude());
                format!(
                    "@{}{prelude} {{ {} }}",
                    grammar_name(payload.grammar()),
                    serialise_rules(payload.nested())
                )
            },
        }
    }
}

fn serialise_qualified_rule(prelude: &str, body: &str) -> String {
    if body.is_empty() {
        format!("{prelude} {{ }}")
    } else {
        format!("{prelude} {{ {body} }}")
    }
}

fn serialise_rule_body(payload: &TypedRulePayload) -> String {
    let declarations = canonical_declaration_block(payload.block().unwrap_or_default());
    let nested = serialise_rules(payload.nested());
    [declarations.as_str(), nested.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn at_rule_prelude(prelude: &str) -> String {
    if prelude.is_empty() {
        String::new()
    } else {
        format!(" {prelude}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InternalStylesheetRoot {
    origin: StyleOrigin,
    rules: Arc<[RuleNode]>,
}

impl InternalStylesheetRoot {
    #[must_use]
    pub fn new(origin: StyleOrigin, rules: impl Into<Arc<[RuleNode]>>) -> Self {
        fn retain_internal_sources(rules: &mut Arc<[RuleNode]>) {
            fn missing(rule: &RuleNode) -> bool {
                rule.grammar() == RuleGrammar::PositionTry && rule.payload().source_stamp.is_none()
                    || rule.payload().nested().iter().any(missing)
            }
            if !rules.iter().any(missing) {
                return;
            }
            for rule in Arc::make_mut(rules) {
                if rule.grammar() == RuleGrammar::PositionTry
                    && rule.payload().source_stamp.is_none()
                {
                    rule.payload_mut().source_stamp =
                        Some(RuleSourceStamp::initial(RuleHandle::allocate()));
                }
                retain_internal_sources(&mut rule.payload_mut().nested);
            }
        }
        let mut rules = rules.into();
        retain_internal_sources(&mut rules);
        Self { origin, rules }
    }

    #[must_use]
    pub const fn origin(&self) -> StyleOrigin {
        self.origin
    }

    #[must_use]
    pub fn rules(&self) -> &[RuleNode] {
        &self.rules
    }

    #[must_use]
    pub fn projection_serialization(&self) -> String {
        self.rules
            .iter()
            .map(RuleNode::projection_serialization)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn canonical_declaration_block(source: &str) -> String {
    let source = source.trim();
    if source.is_empty() || source.ends_with(';') {
        source.to_owned()
    } else {
        format!("{source};")
    }
}

fn serialise_rules(rules: &[RuleNode]) -> String {
    rules
        .iter()
        .map(RuleNode::serialization)
        .collect::<Vec<_>>()
        .join(" ")
}

const fn grammar_name(grammar: RuleGrammar) -> &'static str {
    match grammar {
        RuleGrammar::Style
        | RuleGrammar::Keyframe
        | RuleGrammar::Margin
        | RuleGrammar::NestedDeclarations => "",
        RuleGrammar::Namespace => "namespace",
        RuleGrammar::Import => "import",
        RuleGrammar::Media => "media",
        RuleGrammar::Supports => "supports",
        RuleGrammar::Container => "container",
        RuleGrammar::FontFace => "font-face",
        RuleGrammar::FontFeatureValues => "font-feature-values",
        RuleGrammar::FontPaletteValues => "font-palette-values",
        RuleGrammar::CounterStyle => "counter-style",
        RuleGrammar::Keyframes => "keyframes",
        RuleGrammar::Page => "page",
        RuleGrammar::Property => "property",
        RuleGrammar::LayerBlock | RuleGrammar::LayerStatement => "layer",
        RuleGrammar::Scope => "scope",
        RuleGrammar::StartingStyle => "starting-style",
        RuleGrammar::PositionTry => "position-try",
        RuleGrammar::ColorProfile => "color-profile",
        RuleGrammar::When => "when",
        RuleGrammar::Else => "else",
        RuleGrammar::Document => "document",
        RuleGrammar::CustomMedia => "custom-media",
        RuleGrammar::Region => "region",
        RuleGrammar::Footnote => "footnote",
        RuleGrammar::Sidenote => "sidenote",
        RuleGrammar::BdColour => "-moz-broken-content",
        RuleGrammar::Unknown => "",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheetCandidate {
    source: StyleSheetSourceContext,
    rules: Arc<[RuleNode]>,
    media: Option<Arc<str>>,
    disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheetGraphCandidate {
    sheet: StyleSheetCandidate,
    imports: Arc<[StyleSheetImportCandidate]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheetImportCandidate {
    rule_index: usize,
    resolved_url: Arc<str>,
    child: Option<Box<StyleSheetGraphCandidate>>,
}

impl StyleSheetGraphCandidate {
    #[must_use]
    pub fn new(
        sheet: StyleSheetCandidate,
        imports: impl Into<Arc<[StyleSheetImportCandidate]>>,
    ) -> Self {
        Self {
            sheet,
            imports: imports.into(),
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (StyleSheetCandidate, Arc<[StyleSheetImportCandidate]>) {
        (self.sheet, self.imports)
    }

    #[must_use]
    pub const fn sheet(&self) -> &StyleSheetCandidate {
        &self.sheet
    }

    #[must_use]
    pub fn imports(&self) -> &[StyleSheetImportCandidate] {
        &self.imports
    }
}

impl StyleSheetImportCandidate {
    #[must_use]
    pub fn loaded(
        rule_index: usize,
        resolved_url: impl Into<Arc<str>>,
        child: StyleSheetGraphCandidate,
    ) -> Self {
        Self {
            rule_index,
            resolved_url: resolved_url.into(),
            child: Some(Box::new(child)),
        }
    }

    #[must_use]
    pub fn failed(rule_index: usize, resolved_url: impl Into<Arc<str>>) -> Self {
        Self {
            rule_index,
            resolved_url: resolved_url.into(),
            child: None,
        }
    }
}

impl StyleSheetCandidate {
    #[must_use]
    pub fn new(source: StyleSheetSourceContext, rules: impl Into<Arc<[RuleNode]>>) -> Self {
        Self {
            source,
            rules: rules.into(),
            media: None,
            disabled: false,
        }
    }

    #[must_use]
    pub fn rules(&self) -> Arc<[RuleNode]> {
        self.rules.clone()
    }

    #[must_use]
    pub const fn source(&self) -> &StyleSheetSourceContext {
        &self.source
    }

    #[must_use]
    pub fn with_media(mut self, media: Option<impl Into<Arc<str>>>) -> Self {
        self.media = media.map(Into::into);
        self
    }

    #[must_use]
    pub const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug)]
pub struct StyleSheetLease(Arc<Mutex<StyleSheetRecord>>);
#[derive(Debug)]
struct StyleSheetRecord {
    handle: StyleSheetHandle,
    source: StyleSheetSourceContext,
    top_list: RuleListLease,
    revision: u64,
    media: Option<Arc<str>>,
    disabled: bool,
    detached: bool,
    attachments: Vec<StyleSheetAttachmentHandle>,
}
#[derive(Clone, Debug)]
pub struct StyleSheetAttachmentLease(Arc<Mutex<StyleSheetAttachmentRecord>>);
#[derive(Debug)]
struct StyleSheetAttachmentRecord {
    handle: StyleSheetAttachmentHandle,
    sheet: StyleSheetHandle,
    candidate: StyleSheetAttachmentCandidate,
    revision: u64,
    detached: bool,
}
#[derive(Clone, Debug)]
pub struct ImportBindingLease(Arc<Mutex<ImportBindingRecord>>);
#[derive(Debug)]
struct ImportBindingRecord {
    handle: ImportBindingHandle,
    parent_sheet: StyleSheetHandle,
    parent_rule: RuleHandle,
    context: ImportBindingContext,
    resolved_url: Arc<str>,
    state: ImportBindingLoadState,
    loaded_child: Option<StyleSheetLease>,
    revision: u64,
    detached: bool,
}
#[derive(Clone, Debug)]
pub struct RuleListLease(Arc<Mutex<RuleListRecord>>);
#[derive(Debug)]
struct RuleListRecord {
    handle: RuleListHandle,
    parent_sheet: StyleSheetHandle,
    cssom_parent_sheet: Option<StyleSheetHandle>,
    parent_rule: Option<RuleHandle>,
    binding_context: RuleBindingContext,
    rules: Vec<RuleLease>,
    detached: bool,
}

#[derive(Clone, Copy)]
enum StylesheetRuleOrder {
    EarlyLayers,
    Imports,
    Namespaces,
    Body,
}

impl StylesheetRuleOrder {
    fn advance(self, grammar: RuleGrammar) -> Result<Self, RuleGraphError> {
        match (self, grammar) {
            (Self::EarlyLayers, RuleGrammar::LayerStatement) => Ok(self),
            (Self::EarlyLayers | Self::Imports, RuleGrammar::Import) => Ok(Self::Imports),
            (Self::EarlyLayers | Self::Imports | Self::Namespaces, RuleGrammar::Namespace) => {
                Ok(Self::Namespaces)
            },
            (_, RuleGrammar::Import | RuleGrammar::Namespace) => {
                Err(RuleGraphError::InvalidRuleHierarchy)
            },
            _ => Ok(Self::Body),
        }
    }
}

impl RuleListRecord {
    fn validate_insertion(&self, index: usize, grammar: RuleGrammar) -> Result<(), RuleGraphError> {
        let len = self.rules.len();
        if index > len {
            return Err(RuleGraphError::InvalidInsertionIndex { index, len });
        }
        if self.parent_rule.is_some() {
            if matches!(grammar, RuleGrammar::Import | RuleGrammar::Namespace) {
                return Err(RuleGraphError::InvalidRuleHierarchy);
            }
        } else {
            self.rules[..index]
                .iter()
                .map(|rule| rule.node().grammar())
                .chain(std::iter::once(grammar))
                .chain(self.rules[index..].iter().map(|rule| rule.node().grammar()))
                .try_fold(
                    StylesheetRuleOrder::EarlyLayers,
                    StylesheetRuleOrder::advance,
                )?;
        }
        self.validate_namespace_mutation(grammar)
    }

    fn validate_deletion(&self, index: usize) -> Result<(), RuleGraphError> {
        let rule = self
            .rules
            .get(index)
            .ok_or(RuleGraphError::InvalidDeletionIndex {
                index,
                len: self.rules.len(),
            })?;
        self.validate_namespace_mutation(rule.node().grammar())
    }

    fn validate_namespace_mutation(&self, grammar: RuleGrammar) -> Result<(), RuleGraphError> {
        if grammar == RuleGrammar::Namespace
            && self.rules.iter().any(|rule| {
                !matches!(
                    rule.node().grammar(),
                    RuleGrammar::Import | RuleGrammar::Namespace
                )
            })
        {
            return Err(RuleGraphError::NamespaceMutationForbidden);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RuleLease(Arc<Mutex<RuleRecord>>);
#[derive(Debug)]
struct RuleRecord {
    handle: RuleHandle,
    source_stamp: RuleSourceStamp,
    sheet: StyleSheetHandle,
    binding_context: RuleBindingContext,
    parent_list: Option<RuleListHandle>,
    parent_stylesheet: Option<StyleSheetHandle>,
    node: RuleNode,
    block: Option<RuleBlock>,
    nested_list: Option<RuleListLease>,
    import_bindings: Vec<ImportBindingLease>,
    detached: bool,
}

impl RuleRecord {
    fn mutate(&mut self, mut rule: RuleNode) -> Result<(), RuleGraphError> {
        if self.node.grammar() != rule.grammar() {
            return Err(RuleGraphError::WrongRule);
        }
        rule.payload_mut().nested = self
            .nested_list
            .as_ref()
            .map_or_else(|| Arc::from([]), RuleListLease::nodes);
        if !rule_nodes_have_equal_projection_semantics(&self.node, &rule) {
            self.source_stamp.revision = RuleMutationRevision::next();
        }
        self.block = match (self.block.as_ref(), rule.payload().declaration_block()) {
            (Some(current), Some(block)) => {
                current.replace(self.binding_context, block);
                Some(current.clone())
            },
            (None, Some(block)) => Some(RuleBlock::new(
                self.sheet,
                self.handle,
                self.binding_context,
                block,
            )),
            (_, None) => None,
        };
        self.node = rule;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DetachedRuleLease(RuleLease);

impl DetachedRuleLease {
    pub fn mutate(&self, rule: RuleNode) -> Result<(), RuleGraphError> {
        self.0
            .0
            .lock()
            .expect("rule cell mutex poisoned")
            .mutate(rule)
    }
}

#[derive(Clone, Debug)]
pub struct DetachedRuleListLease(RuleListLease);

impl DetachedRuleListLease {
    pub fn insert(&self, index: usize, rule: RuleNode) -> Result<(), RuleGraphError> {
        let mut record = self.0.0.lock().expect("rule-list cell mutex poisoned");
        record.validate_insertion(index, rule.grammar())?;
        let rule = RuleLease::allocate(
            record.parent_sheet,
            record.handle,
            record.cssom_parent_sheet,
            &rule,
            record.binding_context,
            true,
        );
        record.rules.insert(index, rule);
        Ok(())
    }

    pub fn delete(&self, index: usize) -> Result<(), RuleGraphError> {
        let mut record = self.0.0.lock().expect("rule-list cell mutex poisoned");
        record.validate_deletion(index)?;
        let removed = record.rules.remove(index);
        removed.unlink();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DetachedStyleSheetLease(StyleSheetLease);

impl DetachedStyleSheetLease {
    pub fn set_disabled(&self, disabled: bool) {
        self.0
            .0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .disabled = disabled;
    }

    pub fn set_media(&self, media: Option<&str>) {
        self.0
            .0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .media = media.map(Arc::from);
    }
}

impl StyleSheetLease {
    #[must_use]
    pub fn detached(&self) -> Option<DetachedStyleSheetLease> {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .detached
            .then(|| DetachedStyleSheetLease(self.clone()))
    }

    #[must_use]
    pub fn rule_path(&self, handle: RuleHandle) -> Option<Vec<usize>> {
        self.top_list().path_to_rule(handle)
    }

    #[must_use]
    pub fn same_cell(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn handle(&self) -> StyleSheetHandle {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .handle
    }
    #[must_use]
    pub fn top_list(&self) -> RuleListLease {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .top_list
            .clone()
    }
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .revision
    }
    #[must_use]
    pub fn source(&self) -> StyleSheetSourceContext {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .source
            .clone()
    }
    #[must_use]
    pub fn media(&self) -> Option<Arc<str>> {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .media
            .clone()
    }
    #[must_use]
    pub fn disabled(&self) -> bool {
        self.0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .disabled
    }

    #[must_use]
    pub fn serialise(&self) -> String {
        let top = self.top_list();
        (0..top.len())
            .filter_map(|index| top.rule(index))
            .map(|rule| rule.serialization())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[must_use]
    pub fn serialise_projection_source(&self) -> String {
        let top = self.top_list();
        (0..top.len())
            .filter_map(|index| top.rule(index))
            .map(|rule| rule.projection_serialization())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[must_use]
    pub fn serialise_projection(&self, context: ImportBindingContext) -> String {
        self.serialise_projection_with_stack(context, &mut Vec::new())
    }

    #[must_use]
    pub fn projection_nodes(&self, context: ImportBindingContext) -> Vec<RuleNode> {
        self.projection_nodes_with_stack(context, &mut Vec::new())
    }

    fn projection_nodes_with_stack(
        &self,
        context: ImportBindingContext,
        stack: &mut Vec<StyleSheetHandle>,
    ) -> Vec<RuleNode> {
        if self.disabled() || stack.contains(&self.handle()) {
            return Vec::new();
        }
        stack.push(self.handle());
        let top = self.top_list();
        let mut projected = Vec::new();
        for index in 0..top.len() {
            let Some(rule) = top.rule(index) else {
                continue;
            };
            let node = rule.node();
            let Some(RuleCssomData::Import { request }) = node.cssom_data() else {
                projected.push(
                    rule.snapshot()
                        .with_projection_serialization(rule.projection_serialization()),
                );
                continue;
            };
            let Some(child) = rule
                .import_bindings()
                .into_iter()
                .find(|binding| binding.context() == context)
                .and_then(|binding| binding.loaded_child())
            else {
                continue;
            };
            let mut rules = child.projection_nodes_with_stack(ImportBindingContext::Source, stack);
            if let Some(media) = request.media() {
                rules = vec![RuleNode::media(media, rules)];
            }
            if let Some(supports) = request.supports() {
                rules = vec![RuleNode::supports(supports, rules)];
            }
            rules = match request.layer() {
                RuleImportLayer::Absent => rules,
                RuleImportLayer::Anonymous => vec![RuleNode::layer(None::<Arc<str>>, rules)],
                RuleImportLayer::Named(name) => vec![RuleNode::layer(Some(name.clone()), rules)],
            };
            projected.extend(rules);
        }
        stack.pop();
        projected
    }

    fn serialise_projection_with_stack(
        &self,
        context: ImportBindingContext,
        stack: &mut Vec<StyleSheetHandle>,
    ) -> String {
        if self.disabled() || stack.contains(&self.handle()) {
            return String::new();
        }
        stack.push(self.handle());
        let top = self.top_list();
        let projected = (0..top.len())
            .filter_map(|index| top.rule(index))
            .filter_map(|rule| {
                let node = rule.node();
                let Some(RuleCssomData::Import { request }) = node.cssom_data() else {
                    return Some(rule.projection_serialization());
                };
                let binding = rule
                    .import_bindings()
                    .into_iter()
                    .find(|binding| binding.context() == context)?;
                let child = binding.loaded_child()?;
                let mut css =
                    child.serialise_projection_with_stack(ImportBindingContext::Source, stack);
                if let Some(media) = request.media() {
                    css = format!("@media {media} {{\n{css}\n}}");
                }
                if let Some(supports) = request.supports() {
                    css = format!("@supports {supports} {{\n{css}\n}}");
                }
                if !matches!(request.layer(), RuleImportLayer::Absent) {
                    css = match request.layer() {
                        RuleImportLayer::Absent => unreachable!(),
                        RuleImportLayer::Anonymous => format!("@layer {{\n{css}\n}}"),
                        RuleImportLayer::Named(name) => format!("@layer {name} {{\n{css}\n}}"),
                    };
                }
                Some(css)
            })
            .filter(|css| !css.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        stack.pop();
        projected
    }

    #[must_use]
    pub fn rule_list_at_path(&self, path: &[usize]) -> Option<RuleListLease> {
        let mut list = self.top_list();
        for index in path {
            list = list.rule(*index)?.nested_list()?;
        }
        Some(list)
    }
}

impl StyleSheetAttachmentLease {
    #[must_use]
    pub fn same_cell(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn handle(&self) -> StyleSheetAttachmentHandle {
        self.0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned")
            .handle
    }

    #[must_use]
    pub fn sheet(&self) -> StyleSheetHandle {
        self.0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned")
            .sheet
    }

    #[must_use]
    pub fn candidate(&self) -> StyleSheetAttachmentCandidate {
        self.0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned")
            .candidate
            .clone()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned")
            .revision
    }
}

impl ImportBindingLease {
    #[must_use]
    pub fn same_cell(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn handle(&self) -> ImportBindingHandle {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .handle
    }

    #[must_use]
    pub fn parent_sheet(&self) -> StyleSheetHandle {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .parent_sheet
    }

    #[must_use]
    pub fn parent_rule(&self) -> RuleHandle {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .parent_rule
    }

    #[must_use]
    pub fn context(&self) -> ImportBindingContext {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .context
    }

    #[must_use]
    pub fn resolved_url(&self) -> Arc<str> {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .resolved_url
            .clone()
    }

    #[must_use]
    pub fn state(&self) -> ImportBindingLoadState {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .state
    }

    #[must_use]
    pub fn loaded_child(&self) -> Option<StyleSheetLease> {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .loaded_child
            .clone()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.0
            .lock()
            .expect("import binding cell mutex poisoned")
            .revision
    }
}

impl RuleListLease {
    fn allocate(
        sheet: StyleSheetHandle,
        parent_rule: Option<RuleHandle>,
        cssom_parent_sheet: Option<StyleSheetHandle>,
        nodes: &[RuleNode],
        binding_context: RuleBindingContext,
        detached: bool,
    ) -> Self {
        let list = Self(Arc::new(Mutex::new(RuleListRecord {
            handle: RuleListHandle::allocate(),
            parent_sheet: sheet,
            cssom_parent_sheet,
            parent_rule,
            binding_context,
            rules: Vec::new(),
            detached,
        })));
        let rules = nodes
            .iter()
            .map(|node| {
                RuleLease::allocate(
                    sheet,
                    list.handle(),
                    cssom_parent_sheet,
                    node,
                    binding_context,
                    detached,
                )
            })
            .collect();
        list.0.lock().expect("rule-list cell mutex poisoned").rules = rules;
        list
    }

    #[must_use]
    pub fn detached(&self) -> Option<DetachedRuleListLease> {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .detached
            .then(|| DetachedRuleListLease(self.clone()))
    }

    fn path_to_rule(&self, handle: RuleHandle) -> Option<Vec<usize>> {
        for index in 0..self.len() {
            let rule = self.rule(index)?;
            if rule.handle() == handle {
                return Some(vec![index]);
            }
            if let Some(mut path) = rule
                .nested_list()
                .and_then(|list| list.path_to_rule(handle))
            {
                path.insert(0, index);
                return Some(path);
            }
        }
        None
    }

    #[must_use]
    pub fn handle(&self) -> RuleListHandle {
        self.0.lock().expect("rule-list cell mutex poisoned").handle
    }
    #[must_use]
    pub fn parent_sheet(&self) -> StyleSheetHandle {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .parent_sheet
    }
    #[must_use]
    pub fn parent_rule(&self) -> Option<RuleHandle> {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .parent_rule
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .rules
            .len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[must_use]
    pub fn rule(&self, index: usize) -> Option<RuleLease> {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .rules
            .get(index)
            .cloned()
    }

    #[must_use]
    pub fn nodes(&self) -> Arc<[RuleNode]> {
        self.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .rules
            .iter()
            .map(|rule| rule.snapshot_node())
            .collect::<Vec<_>>()
            .into()
    }
}

impl RuleLease {
    fn allocate(
        sheet: StyleSheetHandle,
        list: RuleListHandle,
        parent_stylesheet: Option<StyleSheetHandle>,
        node: &RuleNode,
        binding_context: RuleBindingContext,
        detached: bool,
    ) -> Self {
        let handle = RuleHandle::allocate();
        let nested_list = node.accepts_nested_rules().then(|| {
            RuleListLease::allocate(
                sheet,
                Some(handle),
                parent_stylesheet,
                node.payload().nested(),
                binding_context,
                detached,
            )
        });
        let block = node
            .payload()
            .declaration_block()
            .map(|block| RuleBlock::new(sheet, handle, binding_context, block));
        Self(Arc::new(Mutex::new(RuleRecord {
            handle,
            source_stamp: node
                .payload()
                .source_stamp
                .unwrap_or_else(|| RuleSourceStamp::initial(handle)),
            sheet,
            binding_context,
            parent_list: Some(list),
            parent_stylesheet,
            node: node.clone(),
            block,
            nested_list,
            import_bindings: Vec::new(),
            detached,
        })))
    }

    #[must_use]
    pub fn detached(&self) -> Option<DetachedRuleLease> {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .detached
            .then(|| DetachedRuleLease(self.clone()))
    }

    #[must_use]
    pub fn handle(&self) -> RuleHandle {
        self.0.lock().expect("rule cell mutex poisoned").handle
    }
    #[must_use]
    pub fn parent_list(&self) -> Option<RuleListHandle> {
        self.0.lock().expect("rule cell mutex poisoned").parent_list
    }

    #[must_use]
    pub fn parent_stylesheet(&self) -> Option<StyleSheetHandle> {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .parent_stylesheet
    }

    fn unlink(&self) {
        let mut record = self.0.lock().expect("rule cell mutex poisoned");
        record.parent_list = None;
        record.parent_stylesheet = None;
        if let Some(list) = &record.nested_list {
            list.0
                .lock()
                .expect("rule-list cell mutex poisoned")
                .cssom_parent_sheet = None;
        }
    }
    #[must_use]
    pub fn node(&self) -> RuleNode {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .node
            .clone()
    }
    #[must_use]
    pub fn snapshot(&self) -> RuleNode {
        self.snapshot_node()
    }
    #[must_use]
    pub fn block(&self) -> Option<RuleBlock> {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .block
            .clone()
    }
    #[must_use]
    pub fn nested_list(&self) -> Option<RuleListLease> {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .nested_list
            .clone()
    }
    #[must_use]
    pub fn import_bindings(&self) -> Vec<ImportBindingLease> {
        self.0
            .lock()
            .expect("rule cell mutex poisoned")
            .import_bindings
            .clone()
    }
    #[must_use]
    pub fn serialization(&self) -> String {
        let record = self.0.lock().expect("rule cell mutex poisoned");
        let mut node = record.node.clone();
        let nested = record.nested_list.clone();
        drop(record);
        let Some(nested) = nested else {
            return node.serialization();
        };
        let Some(header) = node.payload().group_header.as_ref() else {
            node.payload_mut().nested = nested.nodes();
            return node.serialization();
        };
        let children = (0..nested.len())
            .filter_map(|index| nested.rule(index))
            .map(|rule| rule.serialization())
            .collect::<Vec<_>>();
        serialise_cssom_group(
            node.grammar(),
            header,
            node.payload().declaration_block(),
            &children,
        )
    }

    #[must_use]
    pub fn projection_serialization(&self) -> String {
        let record = self.0.lock().expect("rule cell mutex poisoned");
        let projection = record.node.payload().projection_serialization.clone();
        let grammar = record.node.grammar();
        let nested = record.nested_list.clone();
        let header = record.node.payload().group_header.clone();
        let declarations = record.node.payload().declaration_block.clone();
        let original_nested = record.node.payload().nested.clone();
        drop(record);
        let Some(projection) = projection else {
            return self.serialization();
        };
        let Some(nested) = nested else {
            return projection.to_string();
        };
        if nested.is_empty() {
            return projection.to_string();
        }
        if rule_lists_have_equal_projection_semantics(&nested.nodes(), &original_nested) {
            return projection.to_string();
        }
        let Some(header) = header else {
            return self.serialization();
        };
        let children = (0..nested.len())
            .filter_map(|index| nested.rule(index))
            .map(|rule| rule.projection_serialization())
            .collect::<Vec<_>>();
        serialise_cssom_group(grammar, &header, declarations.as_ref(), &children)
    }

    fn snapshot_node(&self) -> RuleNode {
        let record = self.0.lock().expect("rule cell mutex poisoned");
        let source_stamp = record.source_stamp;
        let source = record.node.clone();
        let grammar = source.grammar();
        let nested = record.nested_list.clone();
        let block = record.block.clone();
        let header = source.payload().group_header.clone();
        drop(record);
        let nested_nodes = nested.map_or_else(|| Arc::from([]), |list| list.nodes());
        let mut snapshot = match header {
            Some(header) => RuleNode::authored_with_group_header(
                grammar,
                self.serialization(),
                nested_nodes,
                header,
            ),
            None => RuleNode::authored(grammar, self.serialization(), nested_nodes),
        };
        if let Some(block) = block {
            let declaration_block = block.snapshot();
            snapshot.payload_mut().block = Some(declaration_block.serialization.clone());
            snapshot.payload_mut().declaration_block = Some(declaration_block);
        } else {
            snapshot.payload_mut().declaration_block = source.payload().declaration_block.clone();
        }
        snapshot.payload_mut().cssom_data = source.payload().cssom_data.clone();
        snapshot.payload_mut().projection_serialization =
            source.payload().projection_serialization.clone();
        snapshot.payload_mut().source_stamp = Some(source_stamp);
        snapshot
    }
}

fn rule_lists_have_equal_projection_semantics(left: &[RuleNode], right: &[RuleNode]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| rule_nodes_have_equal_projection_semantics(left, right))
}

fn rule_nodes_have_equal_projection_semantics(left: &RuleNode, right: &RuleNode) -> bool {
    let left_payload = left.payload();
    let right_payload = right.payload();
    if left.grammar() != right.grammar()
        || left_payload.cssom_data != right_payload.cssom_data
        || !declaration_blocks_have_equal_projection_semantics(
            left_payload.declaration_block.as_ref(),
            right_payload.declaration_block.as_ref(),
        )
        || !rule_lists_have_equal_projection_semantics(
            left_payload.nested(),
            right_payload.nested(),
        )
    {
        return false;
    }
    if left_payload.cssom_data.is_some()
        || left_payload.declaration_block.is_some()
        || !left_payload.nested().is_empty()
    {
        return true;
    }
    left.serialization() == right.serialization()
}

fn declaration_blocks_have_equal_projection_semantics(
    left: Option<&RuleDeclarationBlock>,
    right: Option<&RuleDeclarationBlock>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.domain() == right.domain()
                && left.declarations() == right.declarations()
                && left.shorthand_values() == right.shorthand_values()
        },
        (None, None) => true,
        _ => false,
    }
}

fn serialise_cssom_group(
    grammar: RuleGrammar,
    header: &RuleGroupHeader,
    declarations: Option<&RuleDeclarationBlock>,
    children: &[String],
) -> String {
    let header = header.as_str();
    let body = declarations.map_or("", RuleDeclarationBlock::serialization);
    let multiline = match grammar {
        RuleGrammar::Style => !children.is_empty(),
        RuleGrammar::Media
        | RuleGrammar::Supports
        | RuleGrammar::Container
        | RuleGrammar::LayerBlock
        | RuleGrammar::Scope
        | RuleGrammar::StartingStyle
        | RuleGrammar::When
        | RuleGrammar::Else
        | RuleGrammar::Keyframes => true,
        _ => false,
    };
    if !multiline {
        let body = std::iter::once(body)
            .chain(children.iter().map(String::as_str))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        return serialise_qualified_rule(header, &body);
    }
    let mut serialization = format!("{header} {{\n");
    for part in std::iter::once(body)
        .chain(children.iter().map(String::as_str))
        .filter(|part| !part.is_empty())
    {
        serialization.push_str("  ");
        serialization.push_str(part);
        serialization.push('\n');
    }
    serialization.push('}');
    serialization
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleGraphError {
    WrongDocument,
    WrongStylesheet,
    WrongRuleList,
    WrongRule,
    WrongImportBinding,
    ImportBindingAlreadyExists,
    StaleRevision,
    InvalidRuleHierarchy,
    NamespaceMutationForbidden,
    InvalidInsertionIndex { index: usize, len: usize },
    InvalidDeletionIndex { index: usize, len: usize },
}

impl fmt::Display for RuleGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongDocument => "stylesheet belongs to another style document",
            Self::WrongStylesheet => "stylesheet lease does not name a live graph cell",
            Self::WrongRuleList => "rule-list lease does not name a live graph cell",
            Self::WrongRule => "rule lease does not name a live graph cell",
            Self::WrongImportBinding => "import binding lease does not name a live graph cell",
            Self::ImportBindingAlreadyExists => {
                "import rule already has a binding for this context"
            },
            Self::StaleRevision => "stylesheet graph revision is stale",
            Self::InvalidRuleHierarchy => "rule insertion violates stylesheet order or nesting",
            Self::NamespaceMutationForbidden => {
                "namespace rules cannot change when other rule kinds are present"
            },
            Self::InvalidInsertionIndex { .. } => "rule insertion index exceeds rule-list length",
            Self::InvalidDeletionIndex { .. } => "rule deletion index does not name a rule",
        })
    }
}

impl std::error::Error for RuleGraphError {}

#[derive(Debug)]
pub struct PreparedRuleGraphUpdate {
    document: StyleDocumentHandle,
    sheet: StyleSheetHandle,
    base_revision: u64,
    delta: RuleGraphDelta,
}

#[derive(Debug)]
enum RuleGraphDelta {
    Replace {
        rules: Arc<[RuleNode]>,
        metadata: Option<StyleSheetReplacementMetadata>,
    },
    Insert {
        list: RuleListHandle,
        index: usize,
        rule: RuleNode,
    },
    Delete {
        list: RuleListHandle,
        index: usize,
    },
    ReplaceRule {
        list: RuleListHandle,
        index: usize,
        rule: RuleNode,
    },
    MutateRule {
        list: RuleListHandle,
        index: usize,
        rule: RuleNode,
    },
}

#[derive(Debug)]
struct StyleSheetReplacementMetadata {
    source: StyleSheetSourceContext,
    media: Option<Arc<str>>,
    disabled: bool,
}

#[derive(Debug, Default)]
pub(crate) struct StyleGraph {
    sheets: HashMap<StyleSheetHandle, StyleSheetLease>,
    lists: HashMap<RuleListHandle, Weak<Mutex<RuleListRecord>>>,
    rules: HashMap<RuleHandle, Weak<Mutex<RuleRecord>>>,
    attachments: HashMap<StyleSheetAttachmentHandle, StyleSheetAttachmentLease>,
    import_bindings: HashMap<ImportBindingHandle, Weak<Mutex<ImportBindingRecord>>>,
}

impl StyleState {
    pub(crate) fn fork_stylesheets_to(
        &self,
        destination: &mut Self,
    ) -> Vec<(StyleSheetHandle, StyleSheetLease)> {
        let sheets = self
            .stylesheets
            .sheets
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut copies = HashMap::with_capacity(sheets.len());
        for sheet in &sheets {
            let record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
            let mut source = record.source.clone();
            if source.document.is_some() {
                source.document = Some(destination.document);
            }
            let candidate = StyleSheetCandidate {
                source,
                rules: record.top_list.nodes(),
                media: record.media.clone(),
                disabled: record.disabled,
            };
            let copy = destination
                .create_stylesheet(candidate)
                .expect("a validated stylesheet graph must fork");
            copies.insert(record.handle, copy);
        }

        let mut attachment_copies = HashMap::with_capacity(self.stylesheets.attachments.len());
        for attachment in self.stylesheets.attachments.values() {
            let parent = copies
                .get(&attachment.sheet())
                .expect("every attached stylesheet must fork");
            let copy = destination
                .attach_stylesheet(parent, attachment.candidate())
                .expect("a validated stylesheet attachment must fork");
            attachment_copies.insert(attachment.handle(), copy.handle());
        }

        for sheet in &sheets {
            let copy = copies
                .get(&sheet.handle())
                .expect("every stylesheet must have a fork");
            self.fork_import_bindings_for_lists(
                destination,
                &sheet.top_list(),
                &copy.top_list(),
                &copies,
                &attachment_copies,
            );
        }

        copies.into_iter().collect()
    }

    fn fork_import_bindings_for_lists(
        &self,
        destination: &mut Self,
        source: &RuleListLease,
        copy: &RuleListLease,
        sheet_copies: &HashMap<StyleSheetHandle, StyleSheetLease>,
        attachment_copies: &HashMap<StyleSheetAttachmentHandle, StyleSheetAttachmentHandle>,
    ) {
        debug_assert_eq!(source.len(), copy.len());
        for index in 0..source.len() {
            let source_rule = source
                .rule(index)
                .expect("a source rule must remain live while forking");
            let copied_rule = copy
                .rule(index)
                .expect("a copied rule must preserve source topology");
            let bindings = source_rule.import_bindings();
            for binding in bindings {
                let record = binding
                    .0
                    .lock()
                    .expect("import binding cell mutex poisoned");
                if record.detached {
                    continue;
                }
                let context = match record.context {
                    ImportBindingContext::Source => ImportBindingContext::Source,
                    ImportBindingContext::Attachment(handle) => ImportBindingContext::Attachment(
                        *attachment_copies
                            .get(&handle)
                            .expect("every import attachment context must fork"),
                    ),
                };
                let loaded_child = record.loaded_child.as_ref().map(|child| {
                    sheet_copies
                        .get(&child.handle())
                        .expect("every loaded import child must fork")
                        .clone()
                });
                let state = match record.state {
                    ImportBindingLoadState::Pending => ImportBindingLoadState::Pending,
                    ImportBindingLoadState::Failed => ImportBindingLoadState::Failed,
                    ImportBindingLoadState::Loaded(_) => ImportBindingLoadState::Loaded(
                        loaded_child
                            .as_ref()
                            .expect("a loaded import binding must own its child")
                            .handle(),
                    ),
                };
                let handle = ImportBindingHandle::allocate();
                let copied_binding =
                    ImportBindingLease(Arc::new(Mutex::new(ImportBindingRecord {
                        handle,
                        parent_sheet: copy.parent_sheet(),
                        parent_rule: copied_rule.handle(),
                        context,
                        resolved_url: record.resolved_url.clone(),
                        state,
                        loaded_child,
                        revision: record.revision,
                        detached: false,
                    })));
                copied_rule
                    .0
                    .lock()
                    .expect("rule cell mutex poisoned")
                    .import_bindings
                    .push(copied_binding.clone());
                destination
                    .stylesheets
                    .import_bindings
                    .insert(handle, Arc::downgrade(&copied_binding.0));
            }
            match (source_rule.nested_list(), copied_rule.nested_list()) {
                (Some(source_nested), Some(copied_nested)) => self.fork_import_bindings_for_lists(
                    destination,
                    &source_nested,
                    &copied_nested,
                    sheet_copies,
                    attachment_copies,
                ),
                (None, None) => {},
                _ => panic!("a copied stylesheet must preserve nested rule-list topology"),
            }
        }
    }

    pub fn adopt_stylesheet_to(
        &mut self,
        sheet: &StyleSheetLease,
        destination: &mut Self,
    ) -> Result<(), RuleGraphError> {
        let record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let handle = record.handle;
        if destination.stylesheets.sheets.contains_key(&handle) {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if !record.attachments.is_empty() {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let top = record.top_list.clone();
        drop(record);
        self.detach_all_import_bindings(&top);
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        self.remove_rule_list_indexes(&top);
        self.stylesheets.sheets.remove(&handle);
        if record.source.document.is_some() {
            record.source.document = Some(destination.document);
        }
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        drop(record);
        destination.index_rule_list(&top);
        destination.stylesheets.sheets.insert(handle, sheet.clone());
        Ok(())
    }

    pub fn create_stylesheet(
        &mut self,
        candidate: StyleSheetCandidate,
    ) -> Result<StyleSheetLease, RuleGraphError> {
        if candidate
            .source
            .document
            .is_some_and(|document| document != self.document)
        {
            return Err(RuleGraphError::WrongDocument);
        }
        let handle = StyleSheetHandle::allocate();
        let binding_context = binding_context(candidate.source.kind);
        let top_list = self.allocate_rule_list(handle, None, &candidate.rules, binding_context);
        let lease = StyleSheetLease(Arc::new(Mutex::new(StyleSheetRecord {
            handle,
            source: candidate.source,
            top_list,
            revision: 0,
            media: candidate.media,
            disabled: candidate.disabled,
            detached: false,
            attachments: Vec::new(),
        })));
        self.stylesheets.sheets.insert(handle, lease.clone());
        Ok(lease)
    }

    pub fn create_stylesheet_graph(
        &mut self,
        candidate: StyleSheetGraphCandidate,
    ) -> Result<StyleSheetLease, RuleGraphError> {
        let (sheet, imports) = candidate.into_parts();
        self.validate_stylesheet_import_candidates(sheet.source(), &sheet.rules, &imports)?;
        let root = self.create_stylesheet(sheet)?;
        self.bind_stylesheet_import_candidates_unchecked(&root, imports);
        Ok(root)
    }

    pub fn validate_stylesheet_graph_candidate(
        &self,
        candidate: &StyleSheetGraphCandidate,
    ) -> Result<(), RuleGraphError> {
        self.validate_stylesheet_import_candidates(
            candidate.sheet.source(),
            &candidate.sheet.rules,
            &candidate.imports,
        )
    }

    pub fn bind_stylesheet_import_candidates(
        &mut self,
        sheet: &StyleSheetLease,
        imports: Arc<[StyleSheetImportCandidate]>,
    ) -> Result<(), RuleGraphError> {
        let stored = self
            .stylesheets
            .sheets
            .get(&sheet.handle())
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !stored.same_cell(sheet) {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let source = sheet.source();
        let list = sheet.top_list();
        let rules = (0..list.len())
            .map(|index| {
                list.rule(index)
                    .expect("a live rule list must retain each indexed rule")
                    .node()
            })
            .collect::<Vec<_>>();
        self.validate_stylesheet_import_candidates(&source, &rules, &imports)?;
        self.bind_stylesheet_import_candidates_unchecked(sheet, imports);
        Ok(())
    }

    fn validate_stylesheet_import_candidates(
        &self,
        source: &StyleSheetSourceContext,
        rules: &[RuleNode],
        imports: &[StyleSheetImportCandidate],
    ) -> Result<(), RuleGraphError> {
        if source.kind == StyleSheetSourceKind::Constructed
            || source
                .document
                .is_some_and(|document| document != self.document)
        {
            return Err(RuleGraphError::WrongImportBinding);
        }
        let mut indexes = HashSet::new();
        for import in imports {
            if !indexes.insert(import.rule_index)
                || rules.get(import.rule_index).map(RuleNode::grammar) != Some(RuleGrammar::Import)
            {
                return Err(RuleGraphError::WrongImportBinding);
            }
            if let Some(child) = import.child.as_deref() {
                if child.sheet.source.kind != StyleSheetSourceKind::Imported {
                    return Err(RuleGraphError::WrongImportBinding);
                }
                self.validate_stylesheet_import_candidates(
                    &child.sheet.source,
                    &child.sheet.rules,
                    &child.imports,
                )?;
            }
        }
        Ok(())
    }

    fn bind_stylesheet_import_candidates_unchecked(
        &mut self,
        sheet: &StyleSheetLease,
        imports: Arc<[StyleSheetImportCandidate]>,
    ) {
        let list = sheet.top_list();
        for import in imports.iter() {
            let rule = list
                .rule(import.rule_index)
                .expect("a validated import candidate must retain its rule");
            let binding = rule
                .import_bindings()
                .into_iter()
                .find(|binding| binding.context() == ImportBindingContext::Source)
                .unwrap_or_else(|| {
                    self.bind_import(
                        &rule,
                        ImportBindingContext::Source,
                        import.resolved_url.clone(),
                    )
                    .expect("a validated source import candidate must bind")
                });
            match import.child.as_deref().cloned() {
                Some(child) => {
                    let child = self
                        .create_stylesheet_graph(child)
                        .expect("a validated imported stylesheet graph must bind");
                    self.complete_import(&binding, &child)
                        .expect("a validated imported stylesheet must complete its binding");
                },
                None => self
                    .fail_import(&binding)
                    .expect("a validated unavailable import must fail its binding"),
            }
        }
    }

    #[must_use]
    pub fn stylesheet(&self, handle: StyleSheetHandle) -> Option<StyleSheetLease> {
        self.stylesheets.sheets.get(&handle).cloned()
    }
    #[must_use]
    pub fn stylesheet_attachment(
        &self,
        handle: StyleSheetAttachmentHandle,
    ) -> Option<StyleSheetAttachmentLease> {
        self.stylesheets.attachments.get(&handle).cloned()
    }

    pub fn attach_stylesheet(
        &mut self,
        sheet: &StyleSheetLease,
        candidate: StyleSheetAttachmentCandidate,
    ) -> Result<StyleSheetAttachmentLease, RuleGraphError> {
        let mut sheet_record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&sheet_record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || sheet_record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let handle = StyleSheetAttachmentHandle::allocate();
        let attachment =
            StyleSheetAttachmentLease(Arc::new(Mutex::new(StyleSheetAttachmentRecord {
                handle,
                sheet: sheet_record.handle,
                candidate,
                revision: 0,
                detached: false,
            })));
        sheet_record.attachments.push(handle);
        self.stylesheets
            .attachments
            .insert(handle, attachment.clone());
        Ok(attachment)
    }

    pub fn update_stylesheet_attachment(
        &mut self,
        attachment: &StyleSheetAttachmentLease,
        candidate: StyleSheetAttachmentCandidate,
    ) -> Result<bool, RuleGraphError> {
        let mut record = attachment
            .0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned");
        let matches_stored = self
            .stylesheets
            .attachments
            .get(&record.handle)
            .is_some_and(|stored| Arc::ptr_eq(&stored.0, &attachment.0));
        if !matches_stored
            || record.detached
            || !self.stylesheets.sheets.contains_key(&record.sheet)
        {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if record.candidate == candidate {
            return Ok(false);
        }
        let rebuild_imports = attachment_import_environment_changed(&record.candidate, &candidate);
        let sheet_handle = record.sheet;
        let attachment_handle = record.handle;
        record.candidate = candidate;
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet attachment revision space exhausted");
        drop(record);
        if rebuild_imports {
            let top = self
                .stylesheets
                .sheets
                .get(&sheet_handle)
                .expect("a live attachment must name a live stylesheet")
                .top_list();
            self.detach_import_bindings_for_context(
                &top,
                ImportBindingContext::Attachment(attachment_handle),
            );
        }
        Ok(true)
    }

    pub fn detach_stylesheet_attachment(
        &mut self,
        attachment: &StyleSheetAttachmentLease,
    ) -> Result<(), RuleGraphError> {
        let mut record = attachment
            .0
            .lock()
            .expect("stylesheet attachment cell mutex poisoned");
        let matches_stored = self
            .stylesheets
            .attachments
            .get(&record.handle)
            .is_some_and(|stored| Arc::ptr_eq(&stored.0, &attachment.0));
        if !matches_stored || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let sheet = self
            .stylesheets
            .sheets
            .get(&record.sheet)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        let top = sheet.top_list();
        let handle = record.handle;
        sheet
            .0
            .lock()
            .expect("stylesheet cell mutex poisoned")
            .attachments
            .retain(|candidate| *candidate != handle);
        self.stylesheets.attachments.remove(&handle);
        record.detached = true;
        drop(record);
        self.detach_import_bindings_for_context(&top, ImportBindingContext::Attachment(handle));
        Ok(())
    }
    #[must_use]
    pub fn rule(&self, handle: RuleHandle) -> Option<RuleLease> {
        self.stylesheets
            .rules
            .get(&handle)
            .and_then(Weak::upgrade)
            .map(RuleLease)
    }

    #[must_use]
    pub fn import_binding(&self, handle: ImportBindingHandle) -> Option<ImportBindingLease> {
        self.stylesheets
            .import_bindings
            .get(&handle)
            .and_then(Weak::upgrade)
            .map(ImportBindingLease)
            .filter(|binding| {
                !binding
                    .0
                    .lock()
                    .expect("import binding cell mutex poisoned")
                    .detached
            })
    }

    pub fn bind_import(
        &mut self,
        rule: &RuleLease,
        context: ImportBindingContext,
        resolved_url: impl Into<Arc<str>>,
    ) -> Result<ImportBindingLease, RuleGraphError> {
        let mut rule_record = rule.0.lock().expect("rule cell mutex poisoned");
        let stored = self
            .stylesheets
            .rules
            .get(&rule_record.handle)
            .and_then(Weak::upgrade)
            .ok_or(RuleGraphError::WrongRule)?;
        if !Arc::ptr_eq(&stored, &rule.0)
            || rule_record.detached
            || rule_record.node.grammar() != RuleGrammar::Import
        {
            return Err(RuleGraphError::WrongRule);
        }
        let parent_sheet = self
            .live_list(rule_record.parent_list.ok_or(RuleGraphError::WrongRule)?)?
            .parent_sheet();
        let source_kind = self
            .stylesheets
            .sheets
            .get(&parent_sheet)
            .ok_or(RuleGraphError::WrongRule)?
            .source()
            .kind;
        match context {
            ImportBindingContext::Source if source_kind == StyleSheetSourceKind::Constructed => {
                return Err(RuleGraphError::WrongImportBinding);
            },
            ImportBindingContext::Attachment(handle) => {
                if source_kind != StyleSheetSourceKind::Constructed {
                    return Err(RuleGraphError::WrongImportBinding);
                }
                let attachment = self
                    .stylesheets
                    .attachments
                    .get(&handle)
                    .ok_or(RuleGraphError::WrongImportBinding)?;
                if attachment.sheet() != parent_sheet {
                    return Err(RuleGraphError::WrongImportBinding);
                }
            },
            ImportBindingContext::Source => {},
        }
        if rule_record
            .import_bindings
            .iter()
            .any(|binding| binding.context() == context)
        {
            return Err(RuleGraphError::ImportBindingAlreadyExists);
        }
        let handle = ImportBindingHandle::allocate();
        let binding = ImportBindingLease(Arc::new(Mutex::new(ImportBindingRecord {
            handle,
            parent_sheet,
            parent_rule: rule_record.handle,
            context,
            resolved_url: resolved_url.into(),
            state: ImportBindingLoadState::Pending,
            loaded_child: None,
            revision: 0,
            detached: false,
        })));
        rule_record.import_bindings.push(binding.clone());
        self.stylesheets
            .import_bindings
            .insert(handle, Arc::downgrade(&binding.0));
        drop(rule_record);
        self.advance_stylesheet_revision(parent_sheet)?;
        Ok(binding)
    }

    pub fn complete_import(
        &mut self,
        binding: &ImportBindingLease,
        child: &StyleSheetLease,
    ) -> Result<(), RuleGraphError> {
        let mut record = self.live_import_binding(binding)?;
        let parent_sheet = record.parent_sheet;
        let stored_child = self
            .stylesheets
            .sheets
            .get(&child.handle())
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !stored_child.same_cell(child) || child.handle() == record.parent_sheet {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let replaced_child = record
            .loaded_child
            .clone()
            .filter(|loaded| !loaded.same_cell(child));
        record.state = ImportBindingLoadState::Loaded(child.handle());
        record.loaded_child = Some(child.clone());
        record.revision = record
            .revision
            .checked_add(1)
            .expect("import binding revision space exhausted");
        drop(record);
        if let Some(replaced) = replaced_child
            && self.stylesheets.sheets.contains_key(&replaced.handle())
        {
            self.detach_stylesheet(replaced.handle())
                .expect("a live import binding must own its previous child stylesheet");
        }
        self.advance_stylesheet_revision(parent_sheet)?;
        Ok(())
    }

    pub fn fail_import(&mut self, binding: &ImportBindingLease) -> Result<(), RuleGraphError> {
        let mut record = self.live_import_binding(binding)?;
        let parent_sheet = record.parent_sheet;
        let loaded_child = record.loaded_child.clone();
        record.state = ImportBindingLoadState::Failed;
        record.loaded_child = None;
        record.revision = record
            .revision
            .checked_add(1)
            .expect("import binding revision space exhausted");
        drop(record);
        if let Some(child) = loaded_child
            && self.stylesheets.sheets.contains_key(&child.handle())
        {
            self.detach_stylesheet(child.handle())
                .expect("a live import binding must own its previous child stylesheet");
        }
        self.advance_stylesheet_revision(parent_sheet)?;
        Ok(())
    }

    fn advance_stylesheet_revision(
        &mut self,
        handle: StyleSheetHandle,
    ) -> Result<(), RuleGraphError> {
        let sheet = self
            .stylesheets
            .sheets
            .get(&handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        Ok(())
    }

    fn live_import_binding<'a>(
        &self,
        binding: &'a ImportBindingLease,
    ) -> Result<std::sync::MutexGuard<'a, ImportBindingRecord>, RuleGraphError> {
        let record = binding
            .0
            .lock()
            .expect("import binding cell mutex poisoned");
        let matches_stored = self
            .stylesheets
            .import_bindings
            .get(&record.handle)
            .and_then(Weak::upgrade)
            .is_some_and(|stored| Arc::ptr_eq(&stored, &binding.0));
        if !matches_stored || record.detached {
            return Err(RuleGraphError::WrongImportBinding);
        }
        Ok(record)
    }

    pub fn prepare_replace_stylesheet(
        &self,
        sheet: &StyleSheetLease,
        rules: impl Into<Arc<[RuleNode]>>,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        let record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet: record.handle,
            base_revision: record.revision,
            delta: RuleGraphDelta::Replace {
                rules: rules.into(),
                metadata: None,
            },
        })
    }

    pub fn prepare_rebind_stylesheet(
        &self,
        sheet: &StyleSheetLease,
        candidate: StyleSheetCandidate,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        if candidate
            .source
            .document
            .is_some_and(|document| document != self.document)
        {
            return Err(RuleGraphError::WrongDocument);
        }
        let record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet: record.handle,
            base_revision: record.revision,
            delta: RuleGraphDelta::Replace {
                rules: candidate.rules,
                metadata: Some(StyleSheetReplacementMetadata {
                    source: candidate.source,
                    media: candidate.media,
                    disabled: candidate.disabled,
                }),
            },
        })
    }

    pub fn set_stylesheet_disabled(
        &mut self,
        sheet: &StyleSheetLease,
        disabled: bool,
    ) -> Result<bool, RuleGraphError> {
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if record.disabled == disabled {
            return Ok(false);
        }
        record.disabled = disabled;
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        Ok(true)
    }

    pub fn set_stylesheet_media(
        &mut self,
        sheet: &StyleSheetLease,
        media: Option<impl Into<Arc<str>>>,
    ) -> Result<bool, RuleGraphError> {
        let media = media.map(Into::into);
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if record.media == media {
            return Ok(false);
        }
        record.media = media;
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        Ok(true)
    }

    pub fn recontextualize_inline_stylesheet(
        &mut self,
        sheet: &StyleSheetLease,
        base_url: Arc<str>,
        encoding: CssEncoding,
    ) -> Result<bool, RuleGraphError> {
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if record.source.kind != StyleSheetSourceKind::Inline
            || record.source.document != Some(self.document)
        {
            return Ok(false);
        }
        if record.source.base_url.as_ref() == Some(&base_url)
            && record.source.encoding == Some(encoding)
        {
            return Ok(false);
        }
        record.source.base_url = Some(base_url);
        record.source.encoding = Some(encoding);
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        let top = record.top_list.clone();
        drop(record);
        self.detach_import_bindings_for_context(&top, ImportBindingContext::Source);
        Ok(true)
    }

    pub fn prepare_insert_rule(
        &self,
        sheet: &StyleSheetLease,
        list: &RuleListLease,
        index: usize,
        rule: RuleNode,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        let (sheet, base_revision) = self.validate_sheet_and_list(sheet, list)?;
        list.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .validate_insertion(index, rule.grammar())?;
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet,
            base_revision,
            delta: RuleGraphDelta::Insert {
                list: list.handle(),
                index,
                rule,
            },
        })
    }

    pub fn prepare_delete_rule(
        &self,
        sheet: &StyleSheetLease,
        list: &RuleListLease,
        index: usize,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        let (sheet, base_revision) = self.validate_sheet_and_list(sheet, list)?;
        list.0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .validate_deletion(index)?;
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet,
            base_revision,
            delta: RuleGraphDelta::Delete {
                list: list.handle(),
                index,
            },
        })
    }

    pub fn prepare_replace_rule(
        &self,
        sheet: &StyleSheetLease,
        list: &RuleListLease,
        index: usize,
        rule: RuleNode,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        let (sheet, base_revision) = self.validate_sheet_and_list(sheet, list)?;
        let len = list.len();
        if index >= len {
            return Err(RuleGraphError::InvalidDeletionIndex { index, len });
        }
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet,
            base_revision,
            delta: RuleGraphDelta::ReplaceRule {
                list: list.handle(),
                index,
                rule,
            },
        })
    }

    pub fn prepare_mutate_rule(
        &self,
        sheet: &StyleSheetLease,
        list: &RuleListLease,
        index: usize,
        rule: RuleNode,
    ) -> Result<PreparedRuleGraphUpdate, RuleGraphError> {
        let (sheet, base_revision) = self.validate_sheet_and_list(sheet, list)?;
        let current = list
            .rule(index)
            .ok_or(RuleGraphError::InvalidDeletionIndex {
                index,
                len: list.len(),
            })?;
        if current.node().grammar() != rule.grammar() {
            return Err(RuleGraphError::WrongRule);
        }
        Ok(PreparedRuleGraphUpdate {
            document: self.document,
            sheet,
            base_revision,
            delta: RuleGraphDelta::MutateRule {
                list: list.handle(),
                index,
                rule,
            },
        })
    }

    pub fn commit_rule_graph_update(
        &mut self,
        update: PreparedRuleGraphUpdate,
    ) -> Result<(), RuleGraphError> {
        if update.document != self.document {
            return Err(RuleGraphError::WrongDocument);
        }
        let sheet = self
            .stylesheets
            .sheets
            .get(&update.sheet)
            .cloned()
            .ok_or(RuleGraphError::WrongStylesheet)?;
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        if record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        if record.revision != update.base_revision {
            return Err(RuleGraphError::StaleRevision);
        }
        match update.delta {
            RuleGraphDelta::Replace {
                rules: nodes,
                metadata,
            } => {
                let old_rules = {
                    let mut top = record
                        .top_list
                        .0
                        .lock()
                        .expect("rule-list cell mutex poisoned");
                    std::mem::take(&mut top.rules)
                };
                for rule in old_rules {
                    rule.unlink();
                    self.detach_rule_tree(&rule);
                }
                let rules = self.allocate_rules(
                    update.sheet,
                    &record.top_list,
                    &nodes,
                    binding_context(record.source.kind),
                );
                record
                    .top_list
                    .0
                    .lock()
                    .expect("rule-list cell mutex poisoned")
                    .rules = rules;
                if let Some(metadata) = metadata {
                    record.source = metadata.source;
                    record.media = metadata.media;
                    record.disabled = metadata.disabled;
                }
            },
            RuleGraphDelta::Insert { list, index, rule } => {
                let list = self.live_list(list)?;
                let len = list.len();
                if list.parent_sheet() != update.sheet {
                    return Err(RuleGraphError::WrongRuleList);
                }
                if index > len {
                    return Err(RuleGraphError::InvalidInsertionIndex { index, len });
                }
                let inserted = self.allocate_rules(
                    update.sheet,
                    &list,
                    &[rule],
                    binding_context(record.source.kind),
                );
                list.0
                    .lock()
                    .expect("rule-list cell mutex poisoned")
                    .rules
                    .insert(
                        index,
                        inserted.into_iter().next().expect("one rule was allocated"),
                    );
            },
            RuleGraphDelta::Delete { list, index } => {
                let list = self.live_list(list)?;
                let mut list_record = list.0.lock().expect("rule-list cell mutex poisoned");
                if list_record.parent_sheet != update.sheet {
                    return Err(RuleGraphError::WrongRuleList);
                }
                let len = list_record.rules.len();
                if index >= len {
                    return Err(RuleGraphError::InvalidDeletionIndex { index, len });
                }
                let removed = list_record.rules.remove(index);
                drop(list_record);
                removed.unlink();
                self.detach_rule_tree(&removed);
            },
            RuleGraphDelta::ReplaceRule { list, index, rule } => {
                let list = self.live_list(list)?;
                let len = list.len();
                if list.parent_sheet() != update.sheet {
                    return Err(RuleGraphError::WrongRuleList);
                }
                if index >= len {
                    return Err(RuleGraphError::InvalidDeletionIndex { index, len });
                }
                let replacement = self.allocate_rules(
                    update.sheet,
                    &list,
                    &[rule],
                    binding_context(record.source.kind),
                );
                let removed = std::mem::replace(
                    &mut list.0.lock().expect("rule-list cell mutex poisoned").rules[index],
                    replacement
                        .into_iter()
                        .next()
                        .expect("one rule was allocated"),
                );
                removed.unlink();
                self.detach_rule_tree(&removed);
            },
            RuleGraphDelta::MutateRule { list, index, rule } => {
                let list = self.live_list(list)?;
                let mut list_record = list.0.lock().expect("rule-list cell mutex poisoned");
                if list_record.parent_sheet != update.sheet {
                    return Err(RuleGraphError::WrongRuleList);
                }
                let len = list_record.rules.len();
                let current = list_record
                    .rules
                    .get_mut(index)
                    .ok_or(RuleGraphError::InvalidDeletionIndex { index, len })?;
                current
                    .0
                    .lock()
                    .expect("rule cell mutex poisoned")
                    .mutate(rule)?;
            },
        }
        record.revision = record
            .revision
            .checked_add(1)
            .expect("stylesheet revision space exhausted");
        Ok(())
    }

    pub fn detach_stylesheet(&mut self, handle: StyleSheetHandle) -> Result<(), RuleGraphError> {
        let sheet = self
            .stylesheets
            .sheets
            .remove(&handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        let mut record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        record.detached = true;
        for attachment in std::mem::take(&mut record.attachments) {
            if let Some(attachment) = self.stylesheets.attachments.remove(&attachment) {
                attachment
                    .0
                    .lock()
                    .expect("stylesheet attachment cell mutex poisoned")
                    .detached = true;
            }
        }
        let mut top = record
            .top_list
            .0
            .lock()
            .expect("rule-list cell mutex poisoned");
        top.detached = true;
        self.stylesheets.lists.remove(&top.handle);
        for rule in &top.rules {
            self.detach_rule_tree(rule);
        }
        Ok(())
    }

    fn allocate_rule_list(
        &mut self,
        sheet: StyleSheetHandle,
        parent_rule: Option<RuleHandle>,
        nodes: &[RuleNode],
        binding_context: RuleBindingContext,
    ) -> RuleListLease {
        let list = RuleListLease::allocate(
            sheet,
            parent_rule,
            Some(sheet),
            nodes,
            binding_context,
            false,
        );
        self.index_rule_list(&list);
        list
    }

    fn index_rule(&mut self, rule: &RuleLease) {
        self.stylesheets
            .rules
            .insert(rule.handle(), Arc::downgrade(&rule.0));
        if let Some(list) = rule.nested_list() {
            self.index_rule_list(&list);
        }
    }

    fn validate_sheet_and_list(
        &self,
        sheet: &StyleSheetLease,
        list: &RuleListLease,
    ) -> Result<(StyleSheetHandle, u64), RuleGraphError> {
        let record = sheet.0.lock().expect("stylesheet cell mutex poisoned");
        let stored = self
            .stylesheets
            .sheets
            .get(&record.handle)
            .ok_or(RuleGraphError::WrongStylesheet)?;
        if !Arc::ptr_eq(&stored.0, &sheet.0) || record.detached {
            return Err(RuleGraphError::WrongStylesheet);
        }
        let stored_list = self.live_list(list.handle())?;
        if !Arc::ptr_eq(&stored_list.0, &list.0) || list.parent_sheet() != record.handle {
            return Err(RuleGraphError::WrongRuleList);
        }
        Ok((record.handle, record.revision))
    }

    fn live_list(&self, handle: RuleListHandle) -> Result<RuleListLease, RuleGraphError> {
        self.stylesheets
            .lists
            .get(&handle)
            .and_then(Weak::upgrade)
            .map(RuleListLease)
            .filter(|list| {
                !list
                    .0
                    .lock()
                    .expect("rule-list cell mutex poisoned")
                    .detached
            })
            .ok_or(RuleGraphError::WrongRuleList)
    }

    fn allocate_rules(
        &mut self,
        sheet: StyleSheetHandle,
        list: &RuleListLease,
        nodes: &[RuleNode],
        binding_context: RuleBindingContext,
    ) -> Vec<RuleLease> {
        nodes
            .iter()
            .map(|node| {
                let rule = RuleLease::allocate(
                    sheet,
                    list.handle(),
                    Some(sheet),
                    node,
                    binding_context,
                    false,
                );
                self.index_rule(&rule);
                rule
            })
            .collect()
    }

    fn detach_rule_tree(&mut self, rule: &RuleLease) {
        let mut record = rule.0.lock().expect("rule cell mutex poisoned");
        record.detached = true;
        self.stylesheets.rules.remove(&record.handle);
        let nested_list = record.nested_list.clone();
        let bindings = record.import_bindings.clone();
        drop(record);
        for binding in bindings {
            self.detach_import_binding(&binding);
        }
        if let Some(list) = nested_list {
            let mut list_record = list.0.lock().expect("rule-list cell mutex poisoned");
            list_record.detached = true;
            self.stylesheets.lists.remove(&list_record.handle);
            for child in &list_record.rules {
                self.detach_rule_tree(child);
            }
        }
    }

    fn detach_import_binding(&mut self, binding: &ImportBindingLease) {
        let mut record = binding
            .0
            .lock()
            .expect("import binding cell mutex poisoned");
        self.stylesheets.import_bindings.remove(&record.handle);
        record.detached = true;
        let loaded_child = record.loaded_child.clone();
        drop(record);
        if let Some(child) = loaded_child
            && self.stylesheets.sheets.contains_key(&child.handle())
        {
            self.detach_stylesheet(child.handle())
                .expect("a live import binding must own a live child stylesheet");
        }
    }

    fn detach_import_bindings_for_context(
        &mut self,
        list: &RuleListLease,
        context: ImportBindingContext,
    ) {
        let rules = list
            .0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .rules
            .clone();
        for rule in rules {
            let (bindings, nested) = {
                let mut record = rule.0.lock().expect("rule cell mutex poisoned");
                let mut detached = Vec::new();
                record.import_bindings.retain(|binding| {
                    if binding.context() == context {
                        detached.push(binding.clone());
                        false
                    } else {
                        true
                    }
                });
                (detached, record.nested_list.clone())
            };
            for binding in bindings {
                self.detach_import_binding(&binding);
            }
            if let Some(nested) = nested {
                self.detach_import_bindings_for_context(&nested, context);
            }
        }
    }

    fn detach_all_import_bindings(&mut self, list: &RuleListLease) {
        let rules = list
            .0
            .lock()
            .expect("rule-list cell mutex poisoned")
            .rules
            .clone();
        for rule in rules {
            let (bindings, nested) = {
                let mut record = rule.0.lock().expect("rule cell mutex poisoned");
                (
                    std::mem::take(&mut record.import_bindings),
                    record.nested_list.clone(),
                )
            };
            for binding in bindings {
                self.detach_import_binding(&binding);
            }
            if let Some(nested) = nested {
                self.detach_all_import_bindings(&nested);
            }
        }
    }

    fn remove_rule_list_indexes(&mut self, list: &RuleListLease) {
        let record = list.0.lock().expect("rule-list cell mutex poisoned");
        self.stylesheets.lists.remove(&record.handle);
        for rule in &record.rules {
            let rule_record = rule.0.lock().expect("rule cell mutex poisoned");
            self.stylesheets.rules.remove(&rule_record.handle);
            if let Some(nested) = &rule_record.nested_list {
                self.remove_rule_list_indexes(nested);
            }
        }
    }

    fn index_rule_list(&mut self, list: &RuleListLease) {
        let record = list.0.lock().expect("rule-list cell mutex poisoned");
        self.stylesheets
            .lists
            .insert(record.handle, Arc::downgrade(&list.0));
        for rule in &record.rules {
            self.index_rule(rule);
        }
    }
}

const fn binding_context(kind: StyleSheetSourceKind) -> RuleBindingContext {
    match kind {
        StyleSheetSourceKind::Constructed => RuleBindingContext::AttachmentDependent,
        StyleSheetSourceKind::Inline
        | StyleSheetSourceKind::Linked
        | StyleSheetSourceKind::Imported => RuleBindingContext::SourceBound,
    }
}

fn attachment_import_environment_changed(
    previous: &StyleSheetAttachmentCandidate,
    candidate: &StyleSheetAttachmentCandidate,
) -> bool {
    previous.tree_scope != candidate.tree_scope
        || previous.adopter != candidate.adopter
        || previous.environment_revision != candidate.environment_revision
        || previous.base_url != candidate.base_url
        || previous.encoding != candidate.encoding
}
