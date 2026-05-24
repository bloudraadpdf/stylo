/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::context::QuirksMode;
use crate::derives::*;
use crate::error_reporting::{ContextualParseError, ParseErrorReporter};
use crate::media_queries::{Device, MediaList};
use crate::parser::ParserContext;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard};
use crate::stylesheets::loader::StylesheetLoader;
use crate::stylesheets::rule_parser::{State, TopLevelRuleParser};
use crate::stylesheets::rules_iterator::{EffectiveRules, EffectiveRulesIterator};
use crate::stylesheets::rules_iterator::{NestedRuleIterationCondition, RulesIterator};
use crate::stylesheets::{
    CssRule, CssRules, CustomMediaEvaluator, CustomMediaMap, Origin, UrlExtraData,
};
use crate::use_counters::UseCounters;
use crate::{Namespace, Prefix};
use cssparser::{Parser, ParserInput, StyleSheetParser};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use rustc_hash::FxHashMap;
use servo_arc::Arc;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use style_traits::ParsingMode;

use super::scope_rule::ImplicitScopeRoot;

/// This structure holds the user-agent and user stylesheets.
pub struct UserAgentStylesheets {
    /// The lock used for user-agent stylesheets.
    pub shared_lock: SharedRwLock,
    /// The user or user agent stylesheets.
    pub user_or_user_agent_stylesheets: Vec<DocumentStyleSheet>,
    /// The quirks mode stylesheet.
    pub quirks_mode_stylesheet: DocumentStyleSheet,
}

/// A set of namespaces applying to a given stylesheet.
///
/// The namespace id is used in gecko
#[derive(Clone, Debug, Default, MallocSizeOf)]
#[allow(missing_docs)]
pub struct Namespaces {
    pub default: Option<Namespace>,
    pub prefixes: FxHashMap<Prefix, Namespace>,
}

/// The contents of a given stylesheet. This effectively maps to a
/// StyleSheetInner in Gecko.
#[derive(Debug)]
pub struct StylesheetContents {
    /// List of rules in the order they were found (important for
    /// cascading order)
    pub rules: Arc<Locked<CssRules>>,
    /// The origin of this stylesheet.
    pub origin: Origin,
    /// The url data this stylesheet should use.
    pub url_data: UrlExtraData,
    /// The namespaces that apply to this stylesheet.
    pub namespaces: Namespaces,
    /// The quirks mode of this stylesheet.
    pub quirks_mode: QuirksMode,
    /// This stylesheet's source map URL.
    pub source_map_url: Option<String>,
    /// This stylesheet's source URL.
    pub source_url: Option<String>,
    /// The use counters of the original stylesheet.
    pub use_counters: UseCounters,

    /// We don't want to allow construction outside of this file, to guarantee
    /// that all contents are created with Arc<>.
    _forbid_construction: (),
}

impl StylesheetContents {
    /// Parse a given CSS string, with a given url-data, origin, and
    /// quirks mode.
    pub fn from_str(
        css: &str,
        url_data: UrlExtraData,
        origin: Origin,
        shared_lock: &SharedRwLock,
        stylesheet_loader: Option<&dyn StylesheetLoader>,
        error_reporter: Option<&dyn ParseErrorReporter>,
        quirks_mode: QuirksMode,
        allow_import_rules: AllowImportRules,
        sanitization_data: Option<&mut SanitizationData>,
    ) -> Arc<Self> {
        let use_counters = UseCounters::default();
        let (namespaces, rules, source_map_url, source_url) = Stylesheet::parse_rules(
            css,
            &url_data,
            origin,
            &shared_lock,
            stylesheet_loader,
            error_reporter,
            quirks_mode,
            Some(&use_counters),
            allow_import_rules,
            sanitization_data,
        );

        Arc::new(Self {
            rules: CssRules::new(rules, &shared_lock),
            origin,
            url_data,
            namespaces,
            quirks_mode,
            source_map_url,
            source_url,
            use_counters,
            _forbid_construction: (),
        })
    }

    /// Creates a new StylesheetContents with the specified pre-parsed rules,
    /// origin, URL data, and quirks mode.
    ///
    /// Since the rules have already been parsed, and the intention is that
    /// this function is used for read only User Agent style sheets, an empty
    /// namespace map is used, and the source map and source URLs are set to
    /// None.
    ///
    /// An empty namespace map should be fine, as it is only used for parsing,
    /// not serialization of existing selectors.  Since UA sheets are read only,
    /// we should never need the namespace map.
    pub fn from_shared_data(
        rules: Arc<Locked<CssRules>>,
        origin: Origin,
        url_data: UrlExtraData,
        quirks_mode: QuirksMode,
    ) -> Arc<Self> {
        debug_assert!(rules.is_static());
        Arc::new(Self {
            rules,
            origin,
            url_data,
            namespaces: Namespaces::default(),
            quirks_mode,
            source_map_url: None,
            source_url: None,
            use_counters: UseCounters::default(),
            _forbid_construction: (),
        })
    }

    /// Returns a reference to the list of rules.
    #[inline]
    pub fn rules<'a, 'b: 'a>(&'a self, guard: &'b SharedRwLockReadGuard) -> &'a [CssRule] {
        &self.rules.read_with(guard).0
    }

    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        if self.rules.is_static() {
            return 0;
        }
        // Measurement of other fields may be added later.
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }

    /// Return an iterator using the condition `C`.
    #[inline]
    pub fn iter_rules<'a, 'b, C, CMM>(
        &'a self,
        device: &'a Device,
        custom_media: CMM,
        guard: &'a SharedRwLockReadGuard<'b>,
    ) -> RulesIterator<'a, 'b, C, CMM>
    where
        C: NestedRuleIterationCondition,
        CMM: Deref<Target = CustomMediaMap>,
    {
        RulesIterator::new(
            device,
            self.quirks_mode,
            custom_media,
            guard,
            self.rules(guard).iter(),
        )
    }

    /// Return an iterator over the effective rules within the style-sheet, as
    /// according to the supplied `Device`.
    #[inline]
    pub fn effective_rules<'a, 'b, CMM: Deref<Target = CustomMediaMap>>(
        &'a self,
        device: &'a Device,
        custom_media: CMM,
        guard: &'a SharedRwLockReadGuard<'b>,
    ) -> EffectiveRulesIterator<'a, 'b, CMM> {
        self.iter_rules::<EffectiveRules, CMM>(device, custom_media, guard)
    }

    /// Perform a deep clone, of this stylesheet, with an explicit URL data if needed.
    pub fn deep_clone(
        &self,
        lock: &SharedRwLock,
        url_data: Option<&UrlExtraData>,
        guard: &SharedRwLockReadGuard,
    ) -> Arc<Self> {
        // Make a deep clone of the rules, using the new lock.
        let rules = self
            .rules
            .read_with(guard)
            .deep_clone_with_lock(lock, guard);

        let url_data = url_data.cloned().unwrap_or_else(|| self.url_data.clone());

        Arc::new(Self {
            rules: Arc::new(lock.wrap(rules)),
            quirks_mode: self.quirks_mode,
            origin: self.origin,
            url_data,
            namespaces: self.namespaces.clone(),
            source_map_url: self.source_map_url.clone(),
            source_url: self.source_url.clone(),
            use_counters: self.use_counters.clone(),
            _forbid_construction: (),
        })
    }
}

/// The structure servo uses to represent a stylesheet.
#[derive(Debug)]
pub struct Stylesheet {
    /// The contents of this stylesheet.
    pub contents: Locked<Arc<StylesheetContents>>,
    /// The lock used for objects inside this stylesheet
    pub shared_lock: SharedRwLock,
    /// List of media associated with the Stylesheet.
    pub media: Arc<Locked<MediaList>>,
    /// Whether this stylesheet should be disabled.
    pub disabled: AtomicBool,
}

/// A trait to represent a given stylesheet in a document.
pub trait StylesheetInDocument: ::std::fmt::Debug {
    /// Get whether this stylesheet is enabled.
    fn enabled(&self) -> bool;

    /// Get the media associated with this stylesheet.
    fn media<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> Option<&'a MediaList>;

    /// Returns a reference to the contents of the stylesheet.
    fn contents<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> &'a StylesheetContents;

    /// Returns whether the style-sheet applies for the current device.
    fn is_effective_for_device(
        &self,
        device: &Device,
        custom_media: &CustomMediaMap,
        guard: &SharedRwLockReadGuard,
    ) -> bool {
        let media = match self.media(guard) {
            Some(m) => m,
            None => return true,
        };
        media.evaluate(
            device,
            self.contents(guard).quirks_mode,
            &mut CustomMediaEvaluator::new(custom_media, guard),
        )
    }

    /// Return the implicit scope root for this stylesheet, if one exists.
    fn implicit_scope_root(&self) -> Option<ImplicitScopeRoot>;
}

impl StylesheetInDocument for Stylesheet {
    fn media<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> Option<&'a MediaList> {
        Some(self.media.read_with(guard))
    }

    fn enabled(&self) -> bool {
        !self.disabled()
    }

    #[inline]
    fn contents<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> &'a StylesheetContents {
        self.contents.read_with(guard)
    }

    fn implicit_scope_root(&self) -> Option<ImplicitScopeRoot> {
        None
    }
}

/// A simple wrapper over an `Arc<Stylesheet>`, with pointer comparison, and
/// suitable for its use in a `StylesheetSet`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "servo", derive(MallocSizeOf))]
pub struct DocumentStyleSheet(
    #[cfg_attr(feature = "servo", ignore_malloc_size_of = "Arc")] pub Arc<Stylesheet>,
);

impl PartialEq for DocumentStyleSheet {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl StylesheetInDocument for DocumentStyleSheet {
    fn media<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> Option<&'a MediaList> {
        self.0.media(guard)
    }

    fn enabled(&self) -> bool {
        self.0.enabled()
    }

    #[inline]
    fn contents<'a>(&'a self, guard: &'a SharedRwLockReadGuard) -> &'a StylesheetContents {
        self.0.contents(guard)
    }

    fn implicit_scope_root(&self) -> Option<ImplicitScopeRoot> {
        None
    }
}

/// The kind of sanitization to use when parsing a stylesheet.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SanitizationKind {
    /// Perform no sanitization.
    None,
    /// Allow only @font-face, style rules, and @namespace.
    Standard,
    /// Allow everything but conditional rules.
    NoConditionalRules,
}

/// Whether @import rules are allowed.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AllowImportRules {
    /// @import rules will be parsed.
    Yes,
    /// @import rules will not be parsed.
    No,
}

impl SanitizationKind {
    fn allows(self, rule: &CssRule) -> bool {
        debug_assert_ne!(self, SanitizationKind::None);
        // NOTE(emilio): If this becomes more complex (not filtering just by
        // top-level rules), we should thread all the data through nested rules
        // and such. But this doesn't seem necessary at the moment.
        let is_standard = matches!(self, SanitizationKind::Standard);
        match *rule {
            CssRule::Document(..) |
            CssRule::Media(..) |
            CssRule::CustomMedia(..) |
            CssRule::Supports(..) |
            CssRule::When(..) |
            CssRule::Else(..) |
            CssRule::Import(..) |
            CssRule::Container(..) |
            // TODO(emilio): Perhaps Layer should not be always sanitized? But
            // we sanitize @media and co, so this seems safer for now.
            CssRule::LayerStatement(..) |
            CssRule::LayerBlock(..) |
            // TODO(dshin): Same comment as Layer applies - shouldn't give away
            // something like display size - erring on the side of "safe" for now.
            CssRule::Scope(..) |
            CssRule::StartingStyle(..) => false,

            CssRule::FontFace(..) |
            CssRule::Namespace(..) |
            CssRule::Style(..) |
            CssRule::NestedDeclarations(..) |
            CssRule::PositionTry(..) => true,

            CssRule::Keyframes(..) |
            CssRule::Page(..) |
            CssRule::Margin(..) |
            CssRule::Footnote(..) |
            CssRule::Sidenote(..) |
            CssRule::BdColour(..) |
            CssRule::ColorProfile(..) |
            CssRule::Region(..) |
            CssRule::Property(..) |
            CssRule::FontFeatureValues(..) |
            CssRule::FontPaletteValues(..) |
            CssRule::CounterStyle(..) => !is_standard,
        }
    }
}

/// A struct to hold the data relevant to style sheet sanitization.
#[derive(Debug)]
pub struct SanitizationData {
    kind: SanitizationKind,
    output: String,
}

impl SanitizationData {
    /// Create a new input for sanitization.
    #[inline]
    pub fn new(kind: SanitizationKind) -> Option<Self> {
        if matches!(kind, SanitizationKind::None) {
            return None;
        }
        Some(Self {
            kind,
            output: String::new(),
        })
    }

    /// Take the sanitized output.
    #[inline]
    pub fn take(self) -> String {
        self.output
    }
}

impl Stylesheet {
    fn parse_rules(
        css: &str,
        url_data: &UrlExtraData,
        origin: Origin,
        shared_lock: &SharedRwLock,
        stylesheet_loader: Option<&dyn StylesheetLoader>,
        error_reporter: Option<&dyn ParseErrorReporter>,
        quirks_mode: QuirksMode,
        use_counters: Option<&UseCounters>,
        allow_import_rules: AllowImportRules,
        mut sanitization_data: Option<&mut SanitizationData>,
    ) -> (Namespaces, Vec<CssRule>, Option<String>, Option<String>) {
        let mut input = ParserInput::new(css);
        let mut input = Parser::new(&mut input);

        let context = ParserContext::new(
            origin,
            url_data,
            None,
            ParsingMode::DEFAULT,
            quirks_mode,
            /* namespaces = */ Default::default(),
            error_reporter,
            use_counters,
        );

        let mut rule_parser = TopLevelRuleParser {
            shared_lock,
            loader: stylesheet_loader,
            context,
            state: State::Start,
            dom_error: None,
            insert_rule_context: None,
            allow_import_rules,
            declaration_parser_state: Default::default(),
            first_declaration_block: Default::default(),
            wants_first_declaration_block: false,
            error_reporting_state: Default::default(),
            rules: Vec::new(),
        };

        {
            let mut iter = StyleSheetParser::new(&mut input, &mut rule_parser);
            while let Some(result) = iter.next() {
                match result {
                    Ok(rule_start) => {
                        // TODO(emilio, nesting): sanitize nested CSS rules, probably?
                        if let Some(ref mut data) = sanitization_data {
                            if let Some(ref rule) = iter.parser.rules.last() {
                                if !data.kind.allows(rule) {
                                    iter.parser.rules.pop();
                                    continue;
                                }
                            }
                            let end = iter.input.position().byte_index();
                            data.output.push_str(&css[rule_start.byte_index()..end]);
                        }
                    },
                    Err((error, slice)) => {
                        let location = error.location;
                        let error = ContextualParseError::InvalidRule(slice, error);
                        iter.parser.context.log_css_error(location, error);
                    },
                }
            }
        }

        let source_map_url = input.current_source_map_url().map(String::from);
        let source_url = input.current_source_url().map(String::from);
        (
            rule_parser.context.namespaces.into_owned(),
            rule_parser.rules,
            source_map_url,
            source_url,
        )
    }

    /// Creates an empty stylesheet and parses it with a given base url, origin and media.
    pub fn from_str(
        css: &str,
        url_data: UrlExtraData,
        origin: Origin,
        media: Arc<Locked<MediaList>>,
        shared_lock: SharedRwLock,
        stylesheet_loader: Option<&dyn StylesheetLoader>,
        error_reporter: Option<&dyn ParseErrorReporter>,
        quirks_mode: QuirksMode,
        allow_import_rules: AllowImportRules,
    ) -> Self {
        // FIXME: Consider adding use counters to Servo?
        let contents = StylesheetContents::from_str(
            css,
            url_data,
            origin,
            &shared_lock,
            stylesheet_loader,
            error_reporter,
            quirks_mode,
            allow_import_rules,
            /* sanitized_output = */ None,
        );

        Stylesheet {
            contents: shared_lock.wrap(contents),
            shared_lock,
            media,
            disabled: AtomicBool::new(false),
        }
    }

    /// Returns whether the stylesheet has been explicitly disabled through the
    /// CSSOM.
    pub fn disabled(&self) -> bool {
        self.disabled.load(Ordering::SeqCst)
    }

    /// Records that the stylesheet has been explicitly disabled through the
    /// CSSOM.
    ///
    /// Returns whether the the call resulted in a change in disabled state.
    ///
    /// Disabled stylesheets remain in the document, but their rules are not
    /// added to the Stylist.
    pub fn set_disabled(&self, disabled: bool) -> bool {
        self.disabled.swap(disabled, Ordering::SeqCst) != disabled
    }
}

#[cfg(feature = "servo")]
impl Clone for Stylesheet {
    fn clone(&self) -> Self {
        // Create a new lock for our clone.
        let lock = self.shared_lock.clone();
        let guard = self.shared_lock.read();

        // Make a deep clone of the media, using the new lock.
        let media = self.media.read_with(&guard).clone();
        let media = Arc::new(lock.wrap(media));
        let contents = lock.wrap(
            self.contents
                .read_with(&guard)
                .deep_clone(&lock, None, &guard),
        );

        Stylesheet {
            contents,
            media,
            shared_lock: lock,
            disabled: AtomicBool::new(self.disabled.load(Ordering::SeqCst)),
        }
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::color::AbsoluteColor;
    use crate::font_metrics::FontMetrics;
    use crate::media_queries::MediaType;
    use crate::properties::{
        declaration_block::PropertyDeclarationBlock, style_structs::Font, ComputedValues,
        Importance, LonghandId, PropertyDeclaration, StyleBuilder,
    };
    use crate::properties_and_values::value::ComputedValue as ComputedRegisteredValue;
    use crate::queries::values::PrefersColorScheme;
    use crate::servo::media_queries::{Device, FontMetricsProvider};
    use crate::shared_lock::ToCssWithGuard;
    use crate::stylesheets::CssRule;
    use crate::test_support::{pref_lock, BoolPrefGuard};
    use crate::values::computed::font::GenericFontFamily;
    use crate::values::computed::{CSSPixelLength, Length};
    use crate::Atom;
    use cssparser::TokenSerializationType;
    use euclid::{Scale, Size2D};
    use servo_arc::Arc;
    use style_traits::{CSSPixel, DevicePixel, ToCss};

    fn parse_stylesheet(css: &str) -> Stylesheet {
        let shared_lock = SharedRwLock::new();
        let media = Arc::new(shared_lock.wrap(MediaList::empty()));
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        Stylesheet::from_str(
            css,
            url_data,
            Origin::Author,
            media,
            shared_lock,
            None,
            None,
            QuirksMode::NoQuirks,
            AllowImportRules::Yes,
        )
    }

    #[derive(Debug)]
    struct TestFontMetricsProvider;

    impl FontMetricsProvider for TestFontMetricsProvider {
        fn query_font_metrics(
            &self,
            _vertical: bool,
            _font: &Font,
            _base_size: CSSPixelLength,
            _flags: crate::values::specified::font::QueryFontMetricsFlags,
        ) -> FontMetrics {
            FontMetrics::default()
        }

        fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
            Length::new(16.0)
        }
    }

    fn test_stylist() -> crate::stylist::Stylist {
        let default_computed_values =
            ComputedValues::initial_values_with_font_override(Font::initial_values());
        let device = Device::new(
            MediaType::print(),
            QuirksMode::NoQuirks,
            Size2D::<f32, CSSPixel>::new(793.7, 1122.5),
            Scale::<f32, CSSPixel, DevicePixel>::new(1.0),
            Box::new(TestFontMetricsProvider),
            default_computed_values,
            PrefersColorScheme::Light,
        );
        crate::stylist::Stylist::new(device, QuirksMode::NoQuirks)
    }

    fn computed_values_with_custom_length(
        stylist: &crate::stylist::Stylist,
        name: &str,
        css: &str,
        url_data: &UrlExtraData,
    ) -> Arc<ComputedValues> {
        let mut builder =
            StyleBuilder::for_inheritance(stylist.device(), Some(stylist), None, None);
        let value = ComputedRegisteredValue::universal(Arc::new(
            crate::custom_properties::VariableValue::new(
                css.to_owned(),
                url_data,
                TokenSerializationType::Dimension,
                TokenSerializationType::Dimension,
            ),
        ));
        let atom = Atom::from(crate::custom_properties::parse_name(name).unwrap());
        builder.custom_properties.inherited.insert(&atom, value);
        builder.build()
    }

    fn parse_and_compute_color(value: &str) -> crate::values::computed::Color {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);
        let stylist = test_stylist();

        crate::values::specified::color::Color::parse_and_compute(
            &context,
            &mut parser,
            Some(stylist.device()),
        )
        .expect("expected computed color")
    }

    #[test]
    fn servo_parses_page_rules() {
        let stylesheet = parse_stylesheet("@page { size: A4; margin: 1cm; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        assert!(rules.iter().any(|rule| matches!(rule, CssRule::Page(..))));
    }

    #[test]
    fn servo_parses_margin_rules_inside_page() {
        let stylesheet = parse_stylesheet(r#"@page { @top-center { content: "x"; } }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(page) => Some(page.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        assert!(
            nested
                .0
                .iter()
                .any(|rule| matches!(rule, CssRule::Margin(..))),
            "expected nested @margin rule in @page"
        );
    }

    #[test]
    fn servo_preserves_attr_fallback_content_in_pseudo_style_rules() {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);

        let stylesheet = parse_stylesheet(
            r#"p::after { content: " [" attr(data-status string, "unknown") "]"; }"#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let content = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::Content(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected content declaration");
        assert_eq!(
            content, r#"" [" attr(data-status string, "unknown") "]""#,
            "typed style rules should preserve attr() fallback and syntax in pseudo content",
        );
    }

    #[test]
    fn servo_preserves_plain_attr_content_in_pseudo_style_rules() {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);

        let stylesheet = parse_stylesheet(r#"p::after { content: attr(data-label); }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let content = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::Content(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected content declaration");
        assert_eq!(
            content, "attr(data-label)",
            "plain attr() should remain a typed content declaration, not WithVariables",
        );
    }

    #[test]
    fn servo_preserves_text_combine_upright_digits_declaration() {
        let stylesheet = parse_stylesheet("div { text-combine-upright: digits 4; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let text_combine_upright = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::TextCombineUpright(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected text-combine-upright declaration");
        assert_eq!(
            text_combine_upright, "digits 4",
            "typed style rules should preserve text-combine-upright digits values",
        );
    }

    #[test]
    fn servo_preserves_word_space_transform_declaration() {
        let stylesheet = parse_stylesheet("p { word-space-transform: ideographic-space; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let word_space_transform = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::WordSpaceTransform(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected word-space-transform declaration");
        assert_eq!(
            word_space_transform, "ideographic-space",
            "typed style rules should preserve word-space-transform values",
        );
    }

    #[test]
    fn servo_preserves_alignment_baseline_extended_values_declaration() {
        let stylesheet = parse_stylesheet(
            r#"
                .alpha { alignment-baseline: alphabetic; }
                .ideo { alignment-baseline: ideographic; }
                .hang { alignment-baseline: hanging; }
                .central { alignment-baseline: central; }
                .math { alignment-baseline: mathematical; }
            "#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let values: Vec<String> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .filter_map(|style| {
                style
                    .block
                    .read_with(&guard)
                    .declaration_importance_iter()
                    .find_map(|(decl, _)| match decl {
                        PropertyDeclaration::AlignmentBaseline(value) => {
                            Some(value.to_css_string())
                        }
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(
            values,
            vec![
                "alphabetic".to_string(),
                "ideographic".to_string(),
                "hanging".to_string(),
                "central".to_string(),
                "mathematical".to_string(),
            ],
            "typed Servo rules should preserve the full CSS Inline 3 alignment-baseline value set",
        );
    }

    #[test]
    fn servo_preserves_page_float_and_extended_clear_declarations() {
        let stylesheet = parse_stylesheet(
            r#"
                .block-start { float: block-start; }
                .block-end { float: block-end; }
                .snap-bare { float: snap-block; }
                .snap-near { float: snap-block(2em, near); }
                .snap-end { float: snap-block(end); }
                .clear-block-start { clear: block-start; }
                .clear-block-end { clear: block-end; }
                .clear-top { clear: top; }
                .clear-bottom { clear: bottom; }
                .clear-all { clear: all; }
            "#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let mut float_values = Vec::new();
        let mut clear_values = Vec::new();

        for style in rules.iter().filter_map(|rule| match rule {
            CssRule::Style(rule) => Some(rule.read_with(&guard)),
            _ => None,
        }) {
            if let Some(float_value) = style
                .block
                .read_with(&guard)
                .declaration_importance_iter()
                .find_map(|(decl, _)| match decl {
                    PropertyDeclaration::Float(value) => Some(value.to_css_string()),
                    _ => None,
                })
            {
                float_values.push(float_value);
            }
            if let Some(clear_value) = style
                .block
                .read_with(&guard)
                .declaration_importance_iter()
                .find_map(|(decl, _)| match decl {
                    PropertyDeclaration::Clear(value) => Some(value.to_css_string()),
                    _ => None,
                })
            {
                clear_values.push(clear_value);
            }
        }

        assert_eq!(
            float_values,
            vec![
                "block-start".to_string(),
                "block-end".to_string(),
                "snap-block".to_string(),
                "snap-block(2em, near)".to_string(),
                "snap-block(end)".to_string(),
            ],
            "typed Servo rules should preserve logical page-float keywords and snap-block arguments",
        );
        assert_eq!(
            clear_values,
            vec![
                "block-start".to_string(),
                "block-end".to_string(),
                "top".to_string(),
                "bottom".to_string(),
                "all".to_string(),
            ],
            "typed Servo rules should preserve extended page-float clear values",
        );
    }

    #[test]
    fn servo_preserves_dominant_baseline_declaration() {
        let stylesheet = parse_stylesheet(
            r#"
                .hang { dominant-baseline: hanging; }
                .ideo { dominant-baseline: ideographic; }
                .math { dominant-baseline: mathematical; }
            "#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let values: Vec<String> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .filter_map(|style| {
                style
                    .block
                    .read_with(&guard)
                    .declaration_importance_iter()
                    .find_map(|(decl, _)| match decl {
                        PropertyDeclaration::DominantBaseline(value) => {
                            Some(value.to_css_string())
                        }
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(
            values,
            vec![
                "hanging".to_string(),
                "ideographic".to_string(),
                "mathematical".to_string(),
            ],
            "typed Servo rules should preserve dominant-baseline declarations",
        );
    }

    #[test]
    fn servo_preserves_hanging_punctuation_declaration() {
        let stylesheet = parse_stylesheet("p { hanging-punctuation: first allow-end last; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let hanging_punctuation = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::HangingPunctuation(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected hanging-punctuation declaration");
        assert_eq!(
            hanging_punctuation, "first last allow-end",
            "typed style rules should preserve hanging-punctuation values",
        );
    }

    #[test]
    fn servo_preserves_text_indent_hanging_each_line_declaration() {
        let stylesheet = parse_stylesheet("p { text-indent: 2em hanging each-line; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let text_indent = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::TextIndent(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected text-indent declaration");
        assert_eq!(
            text_indent, "2em hanging each-line",
            "text-indent keywords should remain a typed declaration in servo mode",
        );
    }

    #[test]
    fn servo_preserves_system_color_declarations() {
        let stylesheet =
            parse_stylesheet("p { color: CanvasText; background-color: AccentColor; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let declarations: Vec<String> = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .filter_map(|(decl, _)| match decl {
                PropertyDeclaration::Color(value) => Some(value.to_css_string()),
                PropertyDeclaration::BackgroundColor(value) => Some(value.to_css_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            declarations,
            vec!["canvastext".to_string(), "accentcolor".to_string()],
            "typed Servo rules should preserve system-colour keywords rather than requiring source rewrites",
        );
    }

    #[test]
    fn servo_resolves_system_colors_to_print_defaults() {
        let canvas = parse_and_compute_color("Canvas");
        let canvastext = parse_and_compute_color("CanvasText");
        let linktext = parse_and_compute_color("LinkText");

        assert_eq!(
            canvas.resolve_to_absolute(&AbsoluteColor::BLACK),
            AbsoluteColor::WHITE
        );
        assert_eq!(
            canvastext.resolve_to_absolute(&AbsoluteColor::BLACK),
            AbsoluteColor::BLACK
        );
        assert_eq!(
            linktext.resolve_to_absolute(&AbsoluteColor::BLACK),
            AbsoluteColor::srgb_legacy(0, 0, 238, 1.0)
        );
    }

    #[test]
    fn servo_preserves_device_cmyk_authored_declarations() {
        let stylesheet = parse_stylesheet(
            "p { color: device-cmyk(0 1 1 0, red); background-color: device-cmyk(0, 0, 0, 1); }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let declarations: Vec<String> = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .filter_map(|(decl, _)| match decl {
                PropertyDeclaration::Color(value) => Some(value.to_css_string()),
                PropertyDeclaration::BackgroundColor(value) => Some(value.to_css_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            declarations,
            vec![
                "device-cmyk(0 1 1 0, red)".to_string(),
                "device-cmyk(0 0 0 1)".to_string(),
            ],
            "typed Servo rules should preserve device-cmyk() rather than requiring source rewrites",
        );
    }

    #[test]
    fn servo_computes_device_cmyk_with_fallback_colour() {
        let computed = parse_and_compute_color("device-cmyk(0 1 1 0, red)");
        assert_eq!(
            computed.resolve_to_absolute(&AbsoluteColor::BLACK),
            AbsoluteColor::srgb_legacy(255, 0, 0, 1.0)
        );
    }

    #[test]
    fn servo_computes_device_cmyk_alpha_in_modern_and_legacy_forms() {
        let modern = parse_and_compute_color("device-cmyk(0 1 1 0 / 0.5)");
        let modern_resolved = modern.resolve_to_absolute(&AbsoluteColor::BLACK);
        assert_eq!(modern_resolved.into_srgb_legacy().raw_components()[3], 0.5);

        let legacy = parse_and_compute_color("device-cmyk(0, 1, 1, 0, 0.5, red)");
        let legacy_resolved = legacy.resolve_to_absolute(&AbsoluteColor::BLACK);
        assert_eq!(
            legacy_resolved.into_srgb_legacy(),
            AbsoluteColor::srgb_legacy(255, 0, 0, 0.5)
        );
    }

    #[test]
    fn servo_computes_device_cmyk_without_fallback_via_naive_srgb_conversion() {
        let computed = parse_and_compute_color("device-cmyk(0 1 1 0)");
        assert_eq!(
            computed.resolve_to_absolute(&AbsoluteColor::BLACK),
            AbsoluteColor::srgb_legacy(255, 0, 0, 1.0)
        );
    }

    #[test]
    fn servo_preserves_device_cmyk_with_currentcolor_fallback_until_resolution() {
        let computed = parse_and_compute_color("device-cmyk(0 1 1 0, currentcolor)");
        assert_eq!(
            computed.to_css_string(),
            "device-cmyk(0 1 1 0, currentcolor)"
        );
        assert_eq!(
            computed.resolve_to_absolute(&AbsoluteColor::srgb_legacy(0, 0, 255, 1.0)),
            AbsoluteColor::srgb_legacy(0, 0, 255, 1.0)
        );
    }

    #[test]
    fn servo_preserves_font_size_adjust_syntax_distinction() {
        let stylesheet = parse_stylesheet(
            "p.num { font-size-adjust: 0.5; } p.explicit { font-size-adjust: ex-height 0.5; }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let values: Vec<String> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .map(|style| {
                style
                    .block
                    .read_with(&guard)
                    .declaration_importance_iter()
                    .find_map(|(decl, _)| match decl {
                        PropertyDeclaration::FontSizeAdjust(value) => Some(value.to_css_string()),
                        _ => None,
                    })
                    .expect("expected font-size-adjust declaration")
            })
            .collect();
        assert_eq!(
            values,
            vec!["0.5".to_string(), "ex-height 0.5".to_string()],
            "typed style rules should preserve implicit and explicit ex-height syntax distinctly",
        );
    }

    #[test]
    fn servo_preserves_string_list_style_type_declaration() {
        let stylesheet = parse_stylesheet(r#"li { list-style-type: ">> "; }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let list_style_type = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::ListStyleType(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected list-style-type declaration");
        assert_eq!(
            list_style_type, "\">> \"",
            "list-style-type string values should remain typed in servo mode",
        );
    }

    #[test]
    fn servo_preserves_custom_counter_content_in_pseudo_style_rules() {
        let stylesheet =
            parse_stylesheet(r#"p::before { content: "Item " counter(item, bracketed) " / "; }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let content = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::Content(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected content declaration");
        assert_eq!(
            content, r#""Item " counter(item, bracketed) " / ""#,
            "typed style rules should preserve counter() content in pseudo declarations",
        );
    }

    #[test]
    fn servo_resolves_single_longhand_variable_declaration_to_typed_value() {
        let stylesheet = parse_stylesheet("div { margin-top: var(--page-margin); }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let declaration = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| Some(decl.clone()))
            .expect("expected margin-top declaration");

        assert!(
            matches!(declaration, PropertyDeclaration::WithVariables(..)),
            "expected typed rule tree to store unresolved var() longhands as WithVariables"
        );

        let stylist = test_stylist();
        let computed = computed_values_with_custom_length(
            &stylist,
            "--page-margin",
            "25mm",
            &contents.url_data,
        );
        let block = PropertyDeclarationBlock::with_one(declaration, Importance::Normal);
        let resolved = block
            .single_longhand_value_to_declaration(LonghandId::MarginTop, Some(&computed), &stylist)
            .expect("expected typed declaration after variable substitution");

        match &*resolved {
            PropertyDeclaration::MarginTop(value) => {
                assert_eq!(value.to_css_string(), "25mm");
            },
            other => panic!("expected typed margin-top declaration, got {other:?}"),
        }
    }

    #[test]
    fn servo_resolves_env_fallbacks_to_typed_values_when_environment_is_missing() {
        let stylesheet = parse_stylesheet("div { margin-top: env(safe-area-inset-top, 54pt); }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let declaration = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| Some(decl.clone()))
            .expect("expected margin-top declaration");

        assert!(
            matches!(declaration, PropertyDeclaration::WithVariables(..)),
            "expected env() declaration to remain deferred before typed resolution",
        );

        let stylist = test_stylist();
        let computed = stylist.device().default_computed_values();
        let block = PropertyDeclarationBlock::with_one(declaration, Importance::Normal);

        let live = block
            .single_longhand_value_to_declaration(LonghandId::MarginTop, Some(computed), &stylist)
            .expect("expected live environment resolution to succeed");
        let fallback = block
            .single_longhand_value_to_declaration_with_environment_resolution(
                LonghandId::MarginTop,
                Some(computed),
                &stylist,
                crate::custom_properties::EnvironmentResolutionMode::TreatAsMissing,
            )
            .expect("expected authored env fallback resolution to succeed");

        match &*live {
            PropertyDeclaration::MarginTop(value) => {
                assert_eq!(value.to_css_string(), "0px");
            },
            other => panic!("expected typed margin-top declaration, got {other:?}"),
        }

        match &*fallback {
            PropertyDeclaration::MarginTop(value) => {
                assert_eq!(value.to_css_string(), "54pt");
            },
            other => panic!("expected typed margin-top declaration, got {other:?}"),
        }
    }

    #[test]
    fn servo_parses_computed_custom_property_value_as_length_percentage() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let value = ComputedRegisteredValue::universal(Arc::new(
            crate::custom_properties::VariableValue::new(
                "25mm".to_owned(),
                &url_data,
                TokenSerializationType::Dimension,
                TokenSerializationType::Dimension,
            ),
        ));

        let parsed: crate::values::specified::LengthPercentage = value
            .parse_as(QuirksMode::NoQuirks, style_traits::ParsingMode::DEFAULT)
            .expect("expected computed custom property value to parse as a length percentage");
        assert_eq!(parsed.to_css_string(), "25mm");
    }

    #[test]
    fn servo_parses_footnote_rules_inside_page() {
        let _guard = pref_lock().lock().unwrap();
        let _columns_pref = BoolPrefGuard::set("layout.columns.enabled", true);

        let stylesheet = parse_stylesheet(
            "@page { @footnote { border-top: 1pt solid black; column-span: all; max-height: 100pt; } }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(page) => Some(page.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let footnote = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Footnote(rule) => Some(rule),
                _ => None,
            })
            .expect("expected nested @footnote rule");
        assert_eq!(
            footnote.block.read_with(&guard).len(),
            5,
            "border-top should expand to three longhands, alongside column-span and max-height",
        );
    }

    #[test]
    fn servo_parses_background_properties_in_page() {
        let stylesheet =
            parse_stylesheet("@page { background-color: red; background-image: none; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        assert_eq!(
            page.block.read_with(&guard).len(),
            2,
            "background-color and background-image should parse in @page"
        );
    }

    #[test]
    fn servo_parses_box_model_properties_in_page() {
        let stylesheet = parse_stylesheet(
            "@page { padding-top: 1cm; border-top-width: 1px; \
             border-top-style: solid; border-top-color: black; }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        assert_eq!(
            page.block.read_with(&guard).len(),
            4,
            "padding and border longhands should parse in @page"
        );
    }

    #[test]
    fn servo_parses_sizing_properties_in_margin_box() {
        let stylesheet = parse_stylesheet("@page { @top-center { width: 100px; height: 50px; } }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let margin = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Margin(m) => Some(m),
                _ => None,
            })
            .expect("expected @margin rule");
        assert_eq!(
            margin.block.read_with(&guard).len(),
            2,
            "width and height should parse in margin box"
        );
    }

    #[test]
    fn servo_parses_position_and_inset_properties_in_margin_box() {
        let stylesheet = parse_stylesheet(
            "@page { @bottom-right { position: absolute; bottom: 24px; right: 36px; inset-block-start: auto; } }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let margin = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Margin(m) => Some(m),
                _ => None,
            })
            .expect("expected @margin rule");
        assert_eq!(
            margin.block.read_with(&guard).len(),
            4,
            "position plus physical and logical inset properties should parse in margin box",
        );
    }

    #[test]
    fn servo_preserves_env_fallback_in_page_rule_serialization() {
        let stylesheet = parse_stylesheet("@page { margin-top: env(safe-area-inset-top, 12pt); }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page_rule_css = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(_) => Some(rule.to_css_string(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        assert!(
            page_rule_css.contains("env(safe-area-inset-top, 12pt)"),
            "expected serialized @page rule to preserve env() fallback, got: {page_rule_css}",
        );
    }

    #[test]
    fn servo_parses_negative_bleed_in_page_rules_without_parse_errors() {
        let stylesheet = parse_stylesheet("@page { bleed: -3pt; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page_rule_css = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(_) => Some(rule.to_css_string(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        assert!(
            page_rule_css.contains("bleed: -3pt"),
            "expected serialized @page rule to preserve negative bleed, got: {page_rule_css}",
        );
    }

    #[test]
    fn servo_parses_counter_with_lower_roman() {
        let stylesheet =
            parse_stylesheet(r#"@page { @top-center { content: counter(page, lower-roman); } }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let margin = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Margin(m) => Some(m),
                _ => None,
            })
            .expect("expected @margin rule");
        assert_eq!(
            margin.block.read_with(&guard).len(),
            1,
            "content with counter(page, lower-roman) should parse in margin box"
        );
    }

    #[test]
    fn servo_parses_counter_with_upper_roman() {
        let stylesheet =
            parse_stylesheet(r#"@page { @top-center { content: counter(page, upper-roman); } }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let margin = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Margin(m) => Some(m),
                _ => None,
            })
            .expect("expected @margin rule");
        assert_eq!(
            margin.block.read_with(&guard).len(),
            1,
            "content with counter(page, upper-roman) should parse in margin box"
        );
    }

    #[test]
    fn servo_parses_counter_with_decimal_leading_zero() {
        let stylesheet = parse_stylesheet(
            r#"@page { @top-center { content: counter(page, decimal-leading-zero); } }"#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let margin = nested
            .0
            .iter()
            .find_map(|rule| match rule {
                CssRule::Margin(m) => Some(m),
                _ => None,
            })
            .expect("expected @margin rule");
        assert_eq!(
            margin.block.read_with(&guard).len(),
            1,
            "content with counter(page, decimal-leading-zero) should parse in margin box"
        );
    }

    #[test]
    fn servo_parses_break_recto_verso() {
        let stylesheet = parse_stylesheet("div { break-before: recto; break-after: verso; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            2,
            "break-before: recto and break-after: verso should parse"
        );
    }

    #[test]
    fn servo_parses_break_column() {
        let stylesheet = parse_stylesheet("div { break-before: column; break-after: column; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            2,
            "break-before: column and break-after: column should parse",
        );
    }

    #[test]
    fn servo_parses_break_avoid_column() {
        let stylesheet =
            parse_stylesheet("div { break-before: avoid-column; break-after: avoid-column; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            2,
            "break-before: avoid-column and break-after: avoid-column should parse",
        );
    }

    #[test]
    fn servo_parses_break_avoid_region() {
        let stylesheet = parse_stylesheet(
            "div { break-before: avoid-region; break-after: avoid-region; break-inside: avoid-region; }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            3,
            "break-before/break-after/break-inside avoid-region should parse",
        );
    }

    #[test]
    fn servo_parses_column_fill_balance_all() {
        let _columns_pref = BoolPrefGuard::set("layout.columns.enabled", true);
        let stylesheet = parse_stylesheet("div { column-fill: balance-all; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            1,
            "column-fill: balance-all should parse",
        );
    }

    #[test]
    fn servo_parses_bookmark_level() {
        let stylesheet = parse_stylesheet("h1 { bookmark-level: 1; } h2 { bookmark-level: none; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let styles: Vec<_> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .collect();
        assert_eq!(styles.len(), 2, "expected two style rules");
        assert_eq!(
            styles[0].block.read_with(&guard).len(),
            1,
            "bookmark-level: 1 should parse"
        );
        assert_eq!(
            styles[1].block.read_with(&guard).len(),
            1,
            "bookmark-level: none should parse"
        );
    }

    #[test]
    fn servo_parses_bookmark_label() {
        let stylesheet = parse_stylesheet(r#"h1 { bookmark-label: "Chapter " counter(chapter); }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            1,
            "bookmark-label with string + counter() should parse"
        );
    }

    #[test]
    fn servo_parses_bookmark_state() {
        let stylesheet =
            parse_stylesheet("h1 { bookmark-state: open; } h2 { bookmark-state: closed; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let styles: Vec<_> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .collect();
        assert_eq!(styles.len(), 2, "expected two style rules");
        assert_eq!(
            styles[0].block.read_with(&guard).len(),
            1,
            "bookmark-state: open should parse"
        );
        assert_eq!(
            styles[1].block.read_with(&guard).len(),
            1,
            "bookmark-state: closed should parse"
        );
    }

    #[test]
    fn servo_parses_counter_set_in_page() {
        let stylesheet = parse_stylesheet("@page { counter-set: page 1; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        assert_eq!(
            page.block.read_with(&guard).len(),
            1,
            "counter-set should parse in @page context"
        );
    }

    #[test]
    fn servo_parses_container_rules() {
        let stylesheet =
            parse_stylesheet("@container card (width > 10px) { .target { color: red; } }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let container = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Container(rule) => Some(rule),
                _ => None,
            })
            .expect("expected @container rule");
        let nested = container.rules.read_with(&guard);
        assert!(
            nested
                .0
                .iter()
                .any(|rule| matches!(rule, CssRule::Style(..))),
            "expected nested style rule inside @container"
        );
    }

    #[test]
    fn servo_parses_container_relative_units() {
        let stylesheet = parse_stylesheet("div { width: 10cqw; height: 5cqh; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            2,
            "container-relative lengths should parse in Servo mode"
        );
    }

    /// Helper: parse a declaration and assert the round-tripped value.
    /// Used by the moegoe -bd-* fork-extension family round-trip tests
    /// (F6–F12, F22, F28–F31).
    fn assert_bd_roundtrip(
        css: &str,
        property_name: &str,
        expected_value: &str,
    ) {
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let block = style.block.read_with(&guard);
        assert_eq!(
            block.len(),
            1,
            "expected exactly one declaration in `{css}` ({property_name})"
        );
        let mut buf = style_traits::CssString::default();
        block.to_css(&mut buf).expect("serialise block");
        let decl_str: String = buf.into();
        assert!(
            decl_str.contains(property_name),
            "round-tripped block `{decl_str}` should mention `{property_name}`"
        );
        assert!(
            decl_str.contains(expected_value),
            "round-tripped block `{decl_str}` should preserve `{expected_value}`"
        );
    }

    // ----- F6 ----------------------------------------------------------
    #[test]
    fn servo_preserves_bd_footnote_rule_length_declaration() {
        assert_bd_roundtrip(
            "p { -bd-footnote-rule-length: 50%; }",
            "-bd-footnote-rule-length",
            "50%",
        );
    }

    #[test]
    fn servo_preserves_footnote_style_position_declaration() {
        assert_bd_roundtrip(
            "p { footnote-style-position: outside; }",
            "footnote-style-position",
            "outside",
        );
    }

    #[test]
    fn servo_preserves_bd_footnote_fragmentation_declaration() {
        assert_bd_roundtrip(
            "p { -bd-footnote-fragmentation: keep; }",
            "-bd-footnote-fragmentation",
            "keep",
        );
    }

    #[test]
    fn servo_preserves_float_placement_inline_footnote_declaration() {
        assert_bd_roundtrip(
            "p { float-placement: inline-footnote; }",
            "float-placement",
            "inline-footnote",
        );
    }

    // ----- F7 ----------------------------------------------------------
    #[test]
    fn servo_preserves_bd_sidenote_align_declaration() {
        assert_bd_roundtrip(
            "p { -bd-sidenote-align: outside; }",
            "-bd-sidenote-align",
            "outside",
        );
    }

    #[test]
    fn servo_preserves_bd_sidenote_offset_declaration() {
        assert_bd_roundtrip(
            "p { -bd-sidenote-offset: 12pt; }",
            "-bd-sidenote-offset",
            "12pt",
        );
    }

    #[test]
    fn servo_preserves_bd_sidenote_avoid_declaration() {
        assert_bd_roundtrip(
            "p { -bd-sidenote-avoid: caption figure; }",
            "-bd-sidenote-avoid",
            "caption figure",
        );
    }

    // ----- F8 ----------------------------------------------------------
    #[test]
    fn servo_preserves_bd_line_grid_declaration() {
        assert_bd_roundtrip(
            "p { -bd-line-grid: create; }",
            "-bd-line-grid",
            "create",
        );
    }

    #[test]
    fn servo_preserves_bd_baseline_grid_declaration() {
        assert_bd_roundtrip(
            "p { -bd-baseline-grid: 14pt; }",
            "-bd-baseline-grid",
            "14pt",
        );
    }

    // ----- F9 ----------------------------------------------------------
    #[test]
    fn servo_preserves_bd_pdf_destination_declaration() {
        assert_bd_roundtrip(
            r#"p { -bd-pdf-destination: "chapter-1"; }"#,
            "-bd-pdf-destination",
            r#""chapter-1""#,
        );
    }

    #[test]
    fn servo_preserves_bd_destination_area_declaration() {
        assert_bd_roundtrip(
            "p { -bd-destination-area: fit-width; }",
            "-bd-destination-area",
            "fit-width",
        );
    }

    #[test]
    fn servo_preserves_bd_pdf_attachment_location_declaration() {
        assert_bd_roundtrip(
            "p { -bd-pdf-attachment-location: after; }",
            "-bd-pdf-attachment-location",
            "after",
        );
    }

    // ----- F10 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bookmark_target_counter_declaration() {
        assert_bd_roundtrip(
            "p { bookmark-target: 3; }",
            "bookmark-target",
            "3",
        );
    }

    #[test]
    fn servo_preserves_bd_pdf_link_type_declaration() {
        assert_bd_roundtrip(
            "p { -bd-pdf-link-type: embed; }",
            "-bd-pdf-link-type",
            "embed",
        );
    }

    // ----- F11 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_link_declaration() {
        assert_bd_roundtrip(
            "p { -bd-link: none; }",
            "-bd-link",
            "none",
        );
    }

    #[test]
    fn servo_preserves_bd_link_area_declaration() {
        assert_bd_roundtrip(
            "p { -bd-link-area: text; }",
            "-bd-link-area",
            "text",
        );
    }

    // ----- F12 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_text_replace_declaration() {
        assert_bd_roundtrip(
            r#"p { -bd-text-replace: "foo" "bar"; }"#,
            "-bd-text-replace",
            r#""foo""#,
        );
    }

    #[test]
    fn servo_preserves_bd_tooltip_declaration() {
        assert_bd_roundtrip(
            r#"p { -bd-tooltip: "Click for details"; }"#,
            "-bd-tooltip",
            r#""Click for details""#,
        );
    }

    // ----- F22 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_pdf_comment_author_declaration() {
        assert_bd_roundtrip(
            r#"p { -bd-pdf-comment-author: "Alice"; }"#,
            "-bd-pdf-comment-author",
            r#""Alice""#,
        );
    }

    #[test]
    fn servo_preserves_bd_pdf_comment_position_declaration() {
        assert_bd_roundtrip(
            "p { -bd-pdf-comment-position: margin; }",
            "-bd-pdf-comment-position",
            "margin",
        );
    }

    // ----- F28 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_text_wrap_declaration() {
        assert_bd_roundtrip(
            "p { -bd-text-wrap: balance; }",
            "-bd-text-wrap",
            "balance",
        );
    }

    #[test]
    fn servo_preserves_bd_n_lines_declaration() {
        assert_bd_roundtrip(
            "p { -bd-n-lines: 5; }",
            "-bd-n-lines",
            "5",
        );
    }

    // ----- F29 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_margin_inside_declaration() {
        let stylesheet = parse_stylesheet("@page { -bd-margin-inside: 36pt; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let block = page.block.read_with(&guard);
        let mut buf = style_traits::CssString::default();
        block.to_css(&mut buf).expect("serialise page block");
        let decl_str: String = buf.into();
        assert!(
            decl_str.contains("-bd-margin-inside"),
            "page-rule block `{decl_str}` should mention -bd-margin-inside",
        );
        assert!(
            decl_str.contains("36pt"),
            "page-rule block `{decl_str}` should preserve 36pt value",
        );
    }

    // ----- F30 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_page_group_declaration() {
        assert_bd_roundtrip(
            "section { -bd-page-group: start; }",
            "-bd-page-group",
            "start",
        );
    }

    // ----- F31 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bd_hyphenate_limit_lines_declaration() {
        assert_bd_roundtrip(
            "p { -bd-hyphenate-limit-lines: 2; }",
            "-bd-hyphenate-limit-lines",
            "2",
        );
    }

    #[test]
    fn servo_preserves_bd_linebreak_magic_declaration() {
        assert_bd_roundtrip(
            "p { -bd-linebreak-magic: all; }",
            "-bd-linebreak-magic",
            "all",
        );
    }
    /// moegoe F24 — all 12 PDFreactor-compatible proprietary length
    /// units (page-relative and bleed-relative) must parse and
    /// survive into the property block.
    #[test]
    fn servo_parses_bd_page_relative_units() {
        let css = "
            div {
                margin-top: 1-bd-pw;
                margin-bottom: 2-bd-pi;
                margin-left: 3-bd-ph;
                margin-right: 4-bd-pb;
                width: 5-bd-pmin;
                height: 6-bd-pmax;
                padding-top: 7-bd-bw;
                padding-bottom: 8-bd-bi;
                padding-left: 9-bd-bh;
                padding-right: 10-bd-bb;
                min-width: 11-bd-bmin;
                max-width: 12-bd-bmax;
            }
        ";
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            12,
            "every -bd-p* / -bd-b* unit must parse"
        );
    }

    /// moegoe F25 — proprietary counter-style names must survive
    /// the predefined-name case normalisation step (defined in
    /// `style/counter_style/predefined.rs`).
    #[test]
    fn servo_parses_bd_counter_styles() {
        let css = "
            ol.fn { list-style-type: bd-footnote; }
            ol.en { list-style-type: bd-spelled-out-en; }
            ol.en-o { list-style-type: bd-spelled-out-en-ordinal; }
            ol.de { list-style-type: bd-spelled-out-de; }
            ol.fr { list-style-type: bd-spelled-out-fr; }
        ";
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style_rules: Vec<_> = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .collect();
        assert_eq!(style_rules.len(), 5);
        for s in style_rules {
            assert_eq!(
                s.block.read_with(&guard).len(),
                1,
                "every bd-* counter-style name should round-trip"
            );
        }
    }

    /// moegoe F32 — miscellaneous declarative-tuning longhands must
    /// parse and survive into the property block.
    #[test]
    fn servo_parses_bd_misc_longhands() {
        let css = "
            div {
                -bd-lang: \"en-GB\";
                -bd-table-column-span: 3;
                -bd-table-row-span: auto;
                -bd-table-baseline: 2;
                -bd-caption-page: first;
                -bd-target-candidate: yes;
                -bd-truncate-margin-after-break: none;
                -bd-listitem-value: 7;
                -bd-replacedelement: image;
                -bd-scale-content: 0.75;
                -bd-position-origin: padding;
                -bd-line-break-opportunity: before;
                -bd-object-slice: slice;
                -bd-flow: figures;
            }
        ";
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            14,
            "every F32 -bd-* longhand should parse"
        );
    }

    /// moegoe F21 — gap fillers: mask-border-* family, border-clip,
    /// overlay, and text-justify: prince-cjk must all parse.
    #[test]
    fn servo_parses_f21_gap_fillers() {
        let css = "
            div {
                mask-border-source: url(\"mask.png\");
                mask-border-slice: 30 fill;
                mask-border-width: 1;
                mask-border-outset: 0;
                mask-border-repeat: round;
                mask-border-mode: luminance;
                border-clip: clip;
                overlay: auto;
                text-justify: prince-cjk;
            }
        ";
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        assert_eq!(
            style.block.read_with(&guard).len(),
            9,
            "every F21 gap-filler property should parse"
        );
    }

    /// moegoe F21 — the `mask-border` shorthand must parse and
    /// expand into all six mask-border-* longhands.
    #[test]
    fn servo_parses_mask_border_shorthand() {
        let css = "div { mask-border: url(\"mask.png\") 30 / 1 / 0 round luminance; }";
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        // Shorthand expands to six longhands.
        assert_eq!(
            style.block.read_with(&guard).len(),
            6,
            "`mask-border` shorthand should expand into 6 longhands"
        );
    }

    // moegoe Family 14 — `-bd-attr(...)` and `-bd-attr-ancestor(...)`
    // are recognised function names inside `content:` and round-trip
    // back through serialisation. The standard `attr(name, ancestor)`
    // syntax is already covered by the existing tests.
    #[test]
    fn servo_parses_bd_attr_inside_content_value() {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);

        let stylesheet =
            parse_stylesheet(r#"p::after { content: -bd-attr(data-label); }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let content = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::Content(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected content declaration");
        // Self-scope `-bd-attr(...)` serialises back as the standard
        // `attr(...)` (the scope is reflected in the function name on
        // the way out only for the ancestor branch).
        assert_eq!(
            content, "attr(data-label)",
            "-bd-attr(name) should parse as a content item and serialise via the standard attr() spelling"
        );
    }

    #[test]
    fn servo_parses_bd_attr_ancestor_inside_content_value() {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);

        let stylesheet = parse_stylesheet(
            r#"span::before { content: -bd-attr-ancestor(data-section); }"#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let style = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .expect("expected style rule");
        let content = style
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .find_map(|(decl, _)| match decl {
                PropertyDeclaration::Content(value) => Some(value.to_css_string()),
                _ => None,
            })
            .expect("expected content declaration");
        assert_eq!(
            content, "-bd-attr-ancestor(data-section)",
            "-bd-attr-ancestor() should round-trip back to its function-name spelling"
        );
    }

    // moegoe Family 7 — `@-bd-sidenote { … }` nests inside `@page` and
    // accepts an optional flow-name ident in the prelude.
    #[test]
    fn servo_parses_bd_sidenote_rule_inside_page() {
        let _guard = pref_lock().lock().unwrap();

        let stylesheet = parse_stylesheet(
            r#"@page { @-bd-sidenote { width: 80pt; } @-bd-sidenote left { width: 100pt; } }"#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(page) => Some(page.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let nested = page.rules.read_with(&guard);
        let sidenotes: Vec<_> = nested
            .0
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Sidenote(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(
            sidenotes.len(),
            2,
            "expected two nested @-bd-sidenote rules, got {}",
            sidenotes.len()
        );
        assert!(
            sidenotes[0].name.is_none(),
            "unnamed @-bd-sidenote should have no flow name"
        );
        assert_eq!(
            sidenotes[1].name.as_ref().map(|n| n.0.to_string()),
            Some("left".to_string()),
            "@-bd-sidenote left should carry the flow-name ident"
        );
    }

    // moegoe Family 2 — `@-bd-colour <name> { colour-values: … }` parses
    // as a top-level rule and records its declared values + alternate.
    #[test]
    fn servo_parses_bd_colour_at_rule() {
        let stylesheet = parse_stylesheet(
            r#"@-bd-colour PANTONE-185 {
                colour-values: device-cmyk(0 1 0.55 0);
                alternate: cmyk;
            }"#,
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let bd_colour = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::BdColour(r) => Some(r.clone()),
                _ => None,
            })
            .expect("expected @-bd-colour rule");
        assert_eq!(
            bd_colour.name.0.to_string(),
            "PANTONE-185",
            "preserved authored colorant case (PDF 32000-2 §8.6.6.4)"
        );
        assert!(
            bd_colour.values.is_some(),
            "colour-values descriptor parsed into Some(SpecifiedColor)"
        );
        assert!(
            matches!(
                bd_colour.alternate,
                crate::stylesheets::BdColourAlternateKind::Cmyk
            ),
            "alternate descriptor parsed as Cmyk, got {:?}",
            bd_colour.alternate
        );
    }

    // moegoe Family 2 — `@-bd-colour` without `colour-values:` is a
    // parse error (the registry entry would be unusable).
    #[test]
    fn servo_rejects_bd_colour_without_values() {
        let stylesheet = parse_stylesheet(r#"@-bd-colour MissingValues { }"#);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let found = rules
            .iter()
            .any(|rule| matches!(rule, CssRule::BdColour(_)));
        assert!(
            !found,
            "@-bd-colour without colour-values: descriptor must not be admitted"
        );
    }

    fn bd_spot_color_declaration(css: &str) -> Option<String> {
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Style(s) => Some(s.read_with(&guard)),
                _ => None,
            })
            .and_then(|style| {
                style
                    .block
                    .read_with(&guard)
                    .declaration_importance_iter()
                    .find_map(|(decl, _)| match decl {
                        PropertyDeclaration::Color(value) => Some(value.to_css_string()),
                        _ => None,
                    })
            })
    }

    // moegoe Family 2 — author `-bd-spot(<name>)` round-trips as a
    // typed colour function preserved through the cascade.
    #[test]
    fn servo_preserves_bd_spot_colour_function() {
        let serialised = bd_spot_color_declaration("p { color: -bd-spot(PANTONE-185); }")
            .expect("expected color declaration");
        assert_eq!(
            serialised, "-bd-spot(PANTONE-185)",
            "default tint (1) elided per the BdSpot serialiser"
        );
    }

    // moegoe Family 2 — author `-bd-spot(<name>, <tint>)` round-trips
    // with explicit tint preserved.
    #[test]
    fn servo_preserves_bd_spot_with_tint() {
        let serialised =
            bd_spot_color_declaration("p { color: -bd-spot(PANTONE-185, 0.5); }")
                .expect("expected color declaration");
        assert_eq!(
            serialised, "-bd-spot(PANTONE-185, 0.5)",
            "explicit non-unity tint preserved through OM round-trip"
        );
    }

    // moegoe Family 2 — `-bd-separation()` is a synonym for
    // `-bd-spot()`; the authored spelling is preserved through OM
    // round-trips.
    #[test]
    fn servo_preserves_bd_separation_colour_function() {
        let serialised =
            bd_spot_color_declaration("p { color: -bd-separation(PANTONE-185, 0.5); }")
                .expect("expected color declaration");
        assert_eq!(
            serialised, "-bd-separation(PANTONE-185, 0.5)",
            "authored -bd-separation spelling preserved through OM round-trip"
        );
    }

    // moegoe Family 2 — `device-n(<name> <tint>, … , <fallback>)`
    // round-trips with all colorant pairs and the fallback sRGB
    // colour preserved. Spec: CSS Color 5 §4.
    #[test]
    fn servo_preserves_device_n_colour_function() {
        let serialised = bd_spot_color_declaration(
            "p { color: device-n(MyCyan 0.5, MyMagenta 0.3, rgb(0, 0, 0)); }",
        )
        .expect("expected color declaration");
        assert_eq!(
            serialised, "device-n(MyCyan 0.5, MyMagenta 0.3, rgb(0, 0, 0))",
            "device-n colorant pairs and fallback colour preserved through OM round-trip"
        );
    }

    // moegoe Family 2 — `-bd-devicen(...)` is a synonym for
    // `device-n(...)`; the alias serialises to the canonical
    // `device-n(...)` spelling (matches CSS Color 5's
    // vendor-prefix-collapses-to-standard convention).
    #[test]
    fn servo_normalises_bd_devicen_alias_to_device_n() {
        let serialised =
            bd_spot_color_declaration("p { color: -bd-devicen(MyCyan 1, rgb(0, 0, 0)); }")
                .expect("expected color declaration");
        assert_eq!(
            serialised, "device-n(MyCyan 1, rgb(0, 0, 0))",
            "-bd-devicen alias normalises to canonical device-n spelling at serialise time"
        );
    }

    // moegoe Family 30 — `:first-of-group` page pseudo-class.
    #[test]
    fn servo_parses_first_of_group_page_pseudo() {
        let stylesheet =
            parse_stylesheet("@page :first-of-group { margin-top: 5cm; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let page = rules
            .iter()
            .find_map(|rule| match rule {
                CssRule::Page(p) => Some(p.read_with(&guard)),
                _ => None,
            })
            .expect("expected @page rule");
        let selectors = page.selectors.as_slice();
        assert_eq!(
            selectors.len(),
            1,
            "single selector expected, got {}",
            selectors.len()
        );
        let pseudos = &selectors[0].pseudos;
        assert!(
            pseudos
                .iter()
                .any(|pc| matches!(pc, crate::stylesheets::page_rule::PagePseudoClass::FirstOfGroup)),
            "selector should carry the :first-of-group page pseudo"
        );
    }
}
