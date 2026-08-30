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
use cssparser::{Parser, ParserInput, StyleSheetParser, UrlErrorRecovery};
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
        let mut input = ParserInput::new_with_url_error_recovery(css, UrlErrorRecovery::Css2);
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
    use crate::parser::Parse;
    use crate::properties::{
        declaration_block::PropertyDeclarationBlock, style_structs::Font, ComputedValues,
        Importance, LonghandId, PropertyDeclaration, ShorthandId, StyleBuilder,
    };
    use crate::properties_and_values::value::ComputedValue as ComputedRegisteredValue;
    use crate::queries::values::PrefersColorScheme;
    use crate::servo::media_queries::{Device, FontMetricsProvider};
    use crate::shared_lock::ToCssWithGuard;
    use crate::stylesheets::CssRule;
    use crate::test_support::{pref_lock, BoolPrefGuard};
    use crate::values::computed::font::GenericFontFamily;
    use crate::values::computed::{CSSPixelLength, Length, ToComputedValue};
    use crate::Atom;
    use app_units::Au;
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

    fn assert_standard_properties(properties: &[&str]) {
        for property in properties {
            assert!(
                crate::properties::PropertyId::parse_enabled_for_all_content(property).is_ok(),
                "standard property must enter the Servo parser: {property}",
            );
        }
    }

    fn assert_parsed_declaration_count(css: &str, expected: usize) {
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let CssRule::Style(rule) = &contents.rules(&guard)[0] else {
            panic!("expected style rule");
        };
        assert_eq!(
            rule.read_with(&guard).block.read_with(&guard).len(),
            expected
        );
    }

    fn parsed_declarations(
        css: &str,
        name_and_value: impl Fn(&PropertyDeclaration) -> (&'static str, String),
    ) -> Vec<(&'static str, String, Importance)> {
        let _guard = pref_lock().lock().unwrap();
        let _columns_pref = BoolPrefGuard::set("layout.columns.enabled", true);
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let CssRule::Style(rule) = &contents.rules(&guard)[0] else {
            panic!("expected style rule");
        };
        rule.read_with(&guard)
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .map(|(declaration, importance)| {
                let (name, value) = name_and_value(declaration);
                (name, value, importance)
            })
            .collect()
    }

    fn parsed_row_rule_declarations(css: &str) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::RowRuleWidth(value) => ("width", value.to_css_string()),
            PropertyDeclaration::RowRuleStyle(value) => ("style", value.to_css_string()),
            PropertyDeclaration::RowRuleColor(value) => ("color", value.to_css_string()),
            _ => panic!("expected row-rule declaration"),
        })
    }

    fn parsed_rule_break_declarations(css: &str) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::ColumnRuleBreak(value) => ("column", value.to_css_string()),
            PropertyDeclaration::RowRuleBreak(value) => ("row", value.to_css_string()),
            _ => panic!("expected rule-break declaration"),
        })
    }

    fn parsed_rule_visibility_declarations(css: &str) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::ColumnRuleVisibilityItems(value) => {
                ("column", value.to_css_string())
            },
            PropertyDeclaration::RowRuleVisibilityItems(value) => ("row", value.to_css_string()),
            _ => panic!("expected rule-visibility-items declaration"),
        })
    }

    fn parsed_rule_overlap_declarations(css: &str) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::RuleOverlap(value) => ("overlap", value.to_css_string()),
            _ => panic!("expected rule-overlap declaration"),
        })
    }

    fn parsed_bidirectional_rule_declarations(
        css: &str,
    ) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::ColumnRuleColor(value) => ("column-color", value.to_css_string()),
            PropertyDeclaration::RowRuleColor(value) => ("row-color", value.to_css_string()),
            PropertyDeclaration::ColumnRuleStyle(value) => ("column-style", value.to_css_string()),
            PropertyDeclaration::RowRuleStyle(value) => ("row-style", value.to_css_string()),
            PropertyDeclaration::ColumnRuleWidth(value) => ("column-width", value.to_css_string()),
            PropertyDeclaration::RowRuleWidth(value) => ("row-width", value.to_css_string()),
            _ => panic!("expected bidirectional rule declaration"),
        })
    }

    fn parsed_rule_inset_declarations(css: &str) -> Vec<(&'static str, String, Importance)> {
        parsed_declarations(css, |declaration| match declaration {
            PropertyDeclaration::ColumnRuleInsetCapStart(value) => {
                ("column-cap-start", value.to_css_string())
            },
            PropertyDeclaration::ColumnRuleInsetCapEnd(value) => {
                ("column-cap-end", value.to_css_string())
            },
            PropertyDeclaration::ColumnRuleInsetJunctionStart(value) => {
                ("column-junction-start", value.to_css_string())
            },
            PropertyDeclaration::ColumnRuleInsetJunctionEnd(value) => {
                ("column-junction-end", value.to_css_string())
            },
            PropertyDeclaration::RowRuleInsetCapStart(value) => {
                ("row-cap-start", value.to_css_string())
            },
            PropertyDeclaration::RowRuleInsetCapEnd(value) => {
                ("row-cap-end", value.to_css_string())
            },
            PropertyDeclaration::RowRuleInsetJunctionStart(value) => {
                ("row-junction-start", value.to_css_string())
            },
            PropertyDeclaration::RowRuleInsetJunctionEnd(value) => {
                ("row-junction-end", value.to_css_string())
            },
            _ => panic!("expected rule-inset declaration"),
        })
    }

    fn serialized_shorthand(css: &str, shorthand: ShorthandId) -> String {
        let _guard = pref_lock().lock().unwrap();
        let _columns_pref = BoolPrefGuard::set("layout.columns.enabled", true);
        let stylesheet = parse_stylesheet(css);
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let CssRule::Style(rule) = &contents.rules(&guard)[0] else {
            panic!("expected style rule");
        };
        let rule = rule.read_with(&guard);
        let block = rule.block.read_with(&guard);
        let mut output = String::new();
        block.shorthand_to_css(shorthand, &mut output).unwrap();
        output
    }

    #[test]
    fn servo_parses_row_rule_longhands_as_typed_declarations() {
        assert_eq!(
            parsed_row_rule_declarations(
                "p { row-rule-width: 5px; row-rule-style: dashed; row-rule-color: currentcolor; }",
            ),
            vec![
                ("width", "5px".to_string(), Importance::Normal),
                ("style", "dashed".to_string(), Importance::Normal),
                ("color", "currentcolor".to_string(), Importance::Normal),
            ]
        );
    }

    #[test]
    fn servo_row_rule_shorthand_expands_defaults_and_importance() {
        assert_eq!(
            parsed_row_rule_declarations("p { row-rule: dashed !important; }"),
            vec![
                ("width", "medium".to_string(), Importance::Important),
                ("style", "dashed".to_string(), Importance::Important),
                ("color", "currentcolor".to_string(), Importance::Important),
            ]
        );
    }

    #[test]
    fn servo_bidirectional_rule_shorthands_expand_to_both_axes() {
        assert_eq!(
            parsed_bidirectional_rule_declarations(
                "p { rule-color: red; rule-style: solid; rule-width: 10px !important; }",
            ),
            vec![
                ("column-color", "red".to_string(), Importance::Normal),
                ("row-color", "red".to_string(), Importance::Normal),
                ("column-style", "solid".to_string(), Importance::Normal),
                ("row-style", "solid".to_string(), Importance::Normal),
                ("column-width", "10px".to_string(), Importance::Important),
                ("row-width", "10px".to_string(), Importance::Important),
            ]
        );
        for declaration in [
            "p { rule-color: red blue; }",
            "p { rule-style: solid dashed; }",
            "p { rule-width: 1px 2px; }",
        ] {
            assert!(parsed_bidirectional_rule_declarations(declaration).is_empty());
        }
    }

    #[test]
    fn servo_gap_rule_longhands_preserve_list_and_repeater_shape() {
        assert_eq!(
            parsed_bidirectional_rule_declarations(
                "p { \
                 column-rule-color: red, repeat(2, blue, green), repeat(auto, yellow), black; \
                 row-rule-style: dotted, repeat(auto, solid, inset), ridge; \
                 column-rule-width: 2px, repeat(3, thin, 5px), repeat(auto, thick), 8px; \
                 }",
            ),
            vec![
                (
                    "column-color",
                    "red, repeat(2, blue, green), repeat(auto, yellow), black".to_string(),
                    Importance::Normal,
                ),
                (
                    "row-style",
                    "dotted, repeat(auto, solid, inset), ridge".to_string(),
                    Importance::Normal,
                ),
                (
                    "column-width",
                    "2px, repeat(3, thin, 5px), repeat(auto, thick), 8px".to_string(),
                    Importance::Normal,
                ),
            ]
        );

        for declaration in [
            "p { column-rule-color: repeat(auto, red), repeat(auto, blue); }",
            "p { row-rule-style: repeat(0, solid); }",
            "p { column-rule-width: repeat(-1, thin); }",
        ] {
            assert!(parsed_bidirectional_rule_declarations(declaration).is_empty());
        }
    }

    #[test]
    fn servo_rule_shorthand_preserves_full_lists_on_both_axes() {
        assert_eq!(
            parsed_bidirectional_rule_declarations(
                "p { rule: 6px solid red, \
                           repeat(auto, 2px dotted blue), \
                           thick green; }",
            ),
            vec![
                (
                    "column-width",
                    "6px, repeat(auto, 2px), thick".to_string(),
                    Importance::Normal,
                ),
                (
                    "row-width",
                    "6px, repeat(auto, 2px), thick".to_string(),
                    Importance::Normal,
                ),
                (
                    "column-style",
                    "solid, repeat(auto, dotted), none".to_string(),
                    Importance::Normal,
                ),
                (
                    "row-style",
                    "solid, repeat(auto, dotted), none".to_string(),
                    Importance::Normal,
                ),
                (
                    "column-color",
                    "red, repeat(auto, blue), green".to_string(),
                    Importance::Normal,
                ),
                (
                    "row-color",
                    "red, repeat(auto, blue), green".to_string(),
                    Importance::Normal,
                ),
            ]
        );
    }

    #[test]
    fn servo_gap_rule_shorthands_reject_invalid_repeaters() {
        for declaration in [
            "p { rule: repeat(auto, red), repeat(auto, blue); }",
            "p { column-rule: repeat(0, solid); }",
            "p { row-rule: repeat(-1, thin); }",
        ] {
            assert!(parsed_bidirectional_rule_declarations(declaration).is_empty());
        }
    }

    #[test]
    fn servo_gap_rule_shorthands_serialize_complete_rules() {
        let value = "6px solid red, repeat(auto, 2px dotted blue), thick green";
        assert_eq!(
            serialized_shorthand(
                &format!("p {{ column-rule: {value}; }}"),
                ShorthandId::ColumnRule
            ),
            value
        );
        assert_eq!(
            serialized_shorthand(&format!("p {{ row-rule: {value}; }}"), ShorthandId::RowRule),
            value
        );
        assert_eq!(
            serialized_shorthand(&format!("p {{ rule: {value}; }}"), ShorthandId::Rule),
            value
        );
        assert_eq!(
            serialized_shorthand(
                "p { column-rule: 1px solid red; row-rule: 2px solid red; }",
                ShorthandId::Rule,
            ),
            ""
        );
    }

    #[test]
    fn servo_parses_rule_break_longhands_as_typed_declarations() {
        for keyword in ["none", "normal", "intersection"] {
            assert_eq!(
                parsed_rule_break_declarations(&format!(
                    "p {{ column-rule-break: {keyword}; row-rule-break: {keyword}; }}"
                )),
                vec![
                    ("column", keyword.to_string(), Importance::Normal),
                    ("row", keyword.to_string(), Importance::Normal),
                ]
            );
        }
    }

    #[test]
    fn servo_rule_break_shorthand_expands_one_important_value() {
        assert_eq!(
            parsed_rule_break_declarations("p { rule-break: intersection !important; }"),
            vec![
                ("column", "intersection".to_string(), Importance::Important),
                ("row", "intersection".to_string(), Importance::Important),
            ]
        );
    }

    #[test]
    fn servo_rule_break_shorthand_rejects_extra_or_unknown_values() {
        assert!(parsed_rule_break_declarations("p { rule-break: none normal; }").is_empty());
        assert!(parsed_rule_break_declarations("p { rule-break: crossing; }").is_empty());
    }

    #[test]
    fn servo_rule_visibility_items_shorthand_reaches_both_axes() {
        assert_eq!(
            parsed_rule_visibility_declarations("p { rule-visibility-items: between !important; }"),
            vec![
                ("column", "between".to_string(), Importance::Important),
                ("row", "between".to_string(), Importance::Important),
            ]
        );
    }

    #[test]
    fn servo_rule_visibility_items_accepts_only_its_closed_keywords() {
        for keyword in ["all", "around", "between", "normal"] {
            assert_eq!(
                parsed_rule_visibility_declarations(&format!(
                    "p {{ column-rule-visibility-items: {keyword}; \
                          row-rule-visibility-items: {keyword}; }}"
                )),
                vec![
                    ("column", keyword.to_string(), Importance::Normal),
                    ("row", keyword.to_string(), Importance::Normal),
                ]
            );
        }
        for value in ["none", "all around", "between, all"] {
            assert!(parsed_rule_visibility_declarations(&format!(
                "p {{ rule-visibility-items: {value}; }}"
            ))
            .is_empty());
        }
    }

    #[test]
    fn servo_rule_visibility_items_shorthand_serializes_equal_axes() {
        assert_eq!(
            serialized_shorthand(
                "p { rule-visibility-items: around; }",
                ShorthandId::RuleVisibilityItems,
            ),
            "around"
        );
        assert_eq!(
            serialized_shorthand(
                "p { column-rule-visibility-items: all; row-rule-visibility-items: between; }",
                ShorthandId::RuleVisibilityItems,
            ),
            ""
        );
    }

    #[test]
    fn servo_rule_overlap_accepts_only_its_closed_keywords() {
        for keyword in ["row-over-column", "column-over-row"] {
            assert_eq!(
                parsed_rule_overlap_declarations(&format!("p {{ rule-overlap: {keyword}; }}")),
                vec![("overlap", keyword.to_string(), Importance::Normal)]
            );
        }
        for value in ["normal", "column-over-row row-over-column"] {
            assert!(
                parsed_rule_overlap_declarations(&format!("p {{ rule-overlap: {value}; }}"))
                    .is_empty()
            );
        }
    }

    #[test]
    fn servo_rule_inset_longhands_accept_signed_percentages_and_overlap_join() {
        assert_eq!(
            parsed_rule_inset_declarations(
                "p {
                    column-rule-inset-cap-start: -50%;
                    column-rule-inset-cap-end: calc(2px + 25%);
                    row-rule-inset-junction-start: overlap-join;
                }",
            ),
            vec![
                ("column-cap-start", "-50%".to_string(), Importance::Normal),
                (
                    "column-cap-end",
                    "calc(25% + 2px)".to_string(),
                    Importance::Normal,
                ),
                (
                    "row-junction-start",
                    "overlap-join".to_string(),
                    Importance::Normal,
                ),
            ]
        );
    }

    #[test]
    fn servo_rule_inset_intermediate_shorthands_expand_by_shape() {
        assert_eq!(
            parsed_rule_inset_declarations("p { column-rule-inset-start: -50%; }"),
            vec![
                ("column-cap-start", "-50%".to_string(), Importance::Normal),
                (
                    "column-junction-start",
                    "-50%".to_string(),
                    Importance::Normal,
                ),
            ]
        );
        assert_eq!(
            parsed_rule_inset_declarations("p { rule-inset-cap: 1px 2px; }"),
            vec![
                ("column-cap-start", "1px".to_string(), Importance::Normal),
                ("column-cap-end", "2px".to_string(), Importance::Normal),
                ("row-cap-start", "1px".to_string(), Importance::Normal),
                ("row-cap-end", "2px".to_string(), Importance::Normal),
            ]
        );
        assert_eq!(
            parsed_rule_inset_declarations("p { rule-inset-end: overlap-join !important; }"),
            vec![
                (
                    "column-cap-end",
                    "overlap-join".to_string(),
                    Importance::Important,
                ),
                (
                    "column-junction-end",
                    "overlap-join".to_string(),
                    Importance::Important,
                ),
                (
                    "row-cap-end",
                    "overlap-join".to_string(),
                    Importance::Important,
                ),
                (
                    "row-junction-end",
                    "overlap-join".to_string(),
                    Importance::Important,
                ),
            ]
        );
    }

    #[test]
    fn servo_exposes_every_rule_inset_shorthand() {
        for (property, longhand_count) in [
            ("column-rule-inset-start", 2),
            ("column-rule-inset-end", 2),
            ("row-rule-inset-start", 2),
            ("row-rule-inset-end", 2),
            ("rule-inset-start", 4),
            ("rule-inset-end", 4),
            ("column-rule-inset-cap", 2),
            ("column-rule-inset-junction", 2),
            ("row-rule-inset-cap", 2),
            ("row-rule-inset-junction", 2),
            ("rule-inset-cap", 4),
            ("rule-inset-junction", 4),
            ("column-rule-inset", 4),
            ("row-rule-inset", 4),
            ("rule-inset", 8),
        ] {
            let declarations = parsed_rule_inset_declarations(&format!("p {{ {property}: 7px; }}"));
            assert_eq!(declarations.len(), longhand_count, "{property}");
            assert!(
                declarations
                    .iter()
                    .all(|(_, value, importance)| value == "7px"
                        && *importance == Importance::Normal),
                "{property}"
            );
        }
    }

    #[test]
    fn servo_rule_inset_universal_shorthand_expands_slash_grammar() {
        assert_eq!(
            parsed_rule_inset_declarations("p { rule-inset: 1px 2px / 3px 4px !important; }"),
            vec![
                ("column-cap-start", "1px".to_string(), Importance::Important),
                ("column-cap-end", "2px".to_string(), Importance::Important),
                (
                    "column-junction-start",
                    "3px".to_string(),
                    Importance::Important,
                ),
                (
                    "column-junction-end",
                    "4px".to_string(),
                    Importance::Important,
                ),
                ("row-cap-start", "1px".to_string(), Importance::Important),
                ("row-cap-end", "2px".to_string(), Importance::Important),
                (
                    "row-junction-start",
                    "3px".to_string(),
                    Importance::Important,
                ),
                ("row-junction-end", "4px".to_string(), Importance::Important,),
            ]
        );
    }

    #[test]
    fn servo_rule_inset_shorthands_reject_invalid_arity_and_tokens() {
        for value in [
            "auto",
            "normal",
            "1",
            "1px 2px 3px",
            "1px /",
            "1px / 2px 3px 4px",
            "1px / 2px / 3px",
        ] {
            assert!(
                parsed_rule_inset_declarations(&format!("p {{ rule-inset: {value}; }}")).is_empty(),
                "accepted `{value}`"
            );
        }
    }

    #[test]
    fn servo_rule_inset_shorthands_serialize_canonically() {
        assert_eq!(
            serialized_shorthand(
                "p { rule-inset: 1px 2px / 3px 4px; }",
                ShorthandId::RuleInset,
            ),
            "1px 2px / 3px 4px"
        );
        assert_eq!(
            serialized_shorthand("p { rule-inset-cap: 1px 2px; }", ShorthandId::RuleInsetCap),
            "1px 2px"
        );
        assert_eq!(
            serialized_shorthand(
                "p { column-rule-inset: 1px; row-rule-inset: 2px; }",
                ShorthandId::RuleInset,
            ),
            ""
        );
    }

    #[test]
    fn computed_rule_inset_percentage_uses_the_crossing_gap_width() {
        use crate::values::computed::length::RuleInset;
        use crate::values::computed::{LengthPercentage, Percentage};

        let inset = RuleInset::LengthPercentage(LengthPercentage::new_percent(Percentage(-0.5)));
        let RuleInset::LengthPercentage(value) = inset else {
            panic!("expected length-percentage inset");
        };
        assert!(value.has_percentage());
        assert_eq!(value.to_used_value(Au::from_px(20)), Au::from_px(-10));
        assert_eq!(value.to_used_value(Au::new(0)), Au::new(0));
    }

    #[test]
    fn servo_paint_worklet_arguments_with_substitution_are_deferred() {
        let stylesheet = parse_stylesheet(
            ".test { background-image: paint(box, var(--colour), var(--length)); }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let CssRule::Style(rule) = &contents.rules(&guard)[0] else {
            panic!("expected style rule");
        };
        let rule = rule.read_with(&guard);
        let declaration = rule
            .block
            .read_with(&guard)
            .declaration_importance_iter()
            .next()
            .expect("expected background-image declaration")
            .0;

        assert!(matches!(
            declaration,
            PropertyDeclaration::WithVariables(..)
        ));
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
    fn servo_recovers_css2_malformed_urls_at_stylesheet_boundaries() {
        fn background_colors(css: &str, selector: &str) -> Vec<String> {
            use cssparser::ToCss;

            let stylesheet = parse_stylesheet(css);
            let guard = stylesheet.shared_lock.read();
            let contents = stylesheet.contents.read_with(&guard);
            contents
                .rules(&guard)
                .iter()
                .filter_map(|rule| match rule {
                    CssRule::Style(rule) => Some(rule.read_with(&guard)),
                    _ => None,
                })
                .filter(|rule| rule.selectors.to_css_string() == selector)
                .flat_map(|rule| {
                    rule.block
                        .read_with(&guard)
                        .declaration_importance_iter()
                        .filter_map(|(declaration, _)| match declaration {
                            PropertyDeclaration::BackgroundColor(value) => {
                                Some(value.to_css_string())
                            },
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        }

        for (css, selector) in [
            (
                "#three { background-color: green; }\n#foo { background: url(foo\"bar) }\n#three { background-color: red; }",
                "#three",
            ),
            (
                "#foo { background: url(foo\"bar) }\n) }\n#four { background-color: green; }",
                "#four",
            ),
            (
                "#twelve { background: url(}{\"\"{)}); background-color: green; }",
                "#twelve",
            ),
            (
                "#fourteen { background-color: green; }\n#foo { background: url(() }\n#fourteen { background-color: red; }",
                "#fourteen",
            ),
        ] {
            assert_eq!(
                background_colors(css, selector),
                ["green"],
                "CSS 2 malformed-URL recovery must preserve the authored rule boundary for {selector}",
            );
        }
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
                        },
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
                .snap-two { float: snap-block(2em 3em, end); }
                .snap-inline-bare { float: snap-inline; }
                .snap-inline-left { float: snap-inline(4em, left); }
                .snap-inline-two { float: snap-inline(4em 6em, right); }
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
                "snap-block(2em 3em, end)".to_string(),
                "snap-inline".to_string(),
                "snap-inline(4em, left)".to_string(),
                "snap-inline(4em 6em, right)".to_string(),
            ],
            "typed Servo rules should preserve every closed page-float snap shape",
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
    fn servo_rejects_unrepresentable_snap_function_shapes() {
        for value in [
            "snap-block()",
            "snap-block(start)",
            "snap-block(20%, near)",
            "snap-block(calc(1em + 5%), near)",
            "snap-block(2px,)",
            "snap-block(2px, left)",
            "snap-inline()",
            "snap-inline(left)",
            "snap-inline(25%, left)",
            "snap-inline(calc(2pt + 5%), right)",
            "snap-inline(2px, start)",
            "snap-inline(2px 3px, end)",
        ] {
            let stylesheet = parse_stylesheet(&format!(".test {{ float: {value}; }}"));
            let guard = stylesheet.shared_lock.read();
            let contents = stylesheet.contents.read_with(&guard);
            let rules = contents.rules(&guard);
            let has_float = rules
                .iter()
                .filter_map(|rule| match rule {
                    CssRule::Style(rule) => Some(rule.read_with(&guard)),
                    _ => None,
                })
                .any(|style| {
                    style
                        .block
                        .read_with(&guard)
                        .declaration_importance_iter()
                        .any(|(decl, _)| matches!(decl, PropertyDeclaration::Float(_)))
                });
            assert!(!has_float, "invalid float value must be dropped: {value}");
        }
    }

    #[test]
    fn servo_does_not_register_compatibility_names_as_standard_properties() {
        for property in ["snap-block", "snap-inline", "float-defer-page"] {
            assert!(
                crate::properties::PropertyId::parse_enabled_for_all_content(property).is_err(),
                "compatibility property must not enter the standards parser: {property}",
            );
        }
    }

    #[test]
    fn servo_computes_pure_length_snap_calc_without_percentage_state() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new("snap-block(calc(1em + 2pt), near)");
        let mut parser = Parser::new(&mut input);
        let specified = crate::values::specified::box_::Float::parse(&parser_context, &mut parser)
            .expect("a pure-length calc is valid in a snap threshold");
        parser
            .expect_exhausted()
            .expect("the snap function consumes the whole value");

        let stylist = test_stylist();
        let computed = crate::values::computed::Context::for_media_query_evaluation(
            stylist.device(),
            QuirksMode::NoQuirks,
            |context| specified.to_computed_value(context),
        );
        let crate::values::computed::box_::Float::SnapBlock(
            crate::values::generics::box_::GenericSnapBlock::One {
                threshold,
                alignment: Some(crate::values::generics::box_::SnapBlockAlignment::Near),
            },
        ) = computed
        else {
            panic!("expected the one-threshold snap-block computed variant");
        };
        let expected = 16.0 + 2.0 * (96.0 / 72.0);
        assert!(
            (threshold.px() - expected).abs() < 0.001,
            "pure-length calc must compute to a Length: {threshold:?}",
        );
    }

    #[test]
    fn servo_censors_non_finite_snap_thresholds_at_the_computed_boundary() {
        use crate::values::computed::length::MAX_FINITE_CSS_LENGTH_PX;

        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let stylist = test_stylist();

        for (css, expected) in [
            (
                "snap-block(calc(infinity * 1px), near)",
                MAX_FINITE_CSS_LENGTH_PX,
            ),
            (
                "snap-block(calc(-infinity * 1px), near)",
                -MAX_FINITE_CSS_LENGTH_PX,
            ),
            ("snap-block(calc(NaN * 1px), near)", 0.0),
            ("snap-block(calc(-0 * 1px), near)", 0.0),
        ] {
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            let specified =
                crate::values::specified::box_::Float::parse(&parser_context, &mut parser)
                    .expect("CSS Values 4 math constants are valid authored lengths");
            parser
                .expect_exhausted()
                .expect("float value is fully consumed");
            let computed = crate::values::computed::Context::for_media_query_evaluation(
                stylist.device(),
                QuirksMode::NoQuirks,
                |context| specified.to_computed_value(context),
            );
            let crate::values::computed::box_::Float::SnapBlock(
                crate::values::generics::box_::GenericSnapBlock::One { threshold, .. },
            ) = computed
            else {
                panic!("expected the one-threshold snap-block computed variant");
            };
            assert_eq!(threshold.px(), expected, "computed threshold for `{css}`");
            assert!(
                threshold.px().is_finite(),
                "`{css}` must not leak non-finite geometry"
            );
            if expected == 0.0 {
                assert!(
                    threshold.px().is_sign_positive(),
                    "zero is censored to +0 for `{css}`"
                );
            }
        }
    }

    #[test]
    fn servo_preserves_signed_float_offset_through_computation() {
        let stylesheet = parse_stylesheet(".test { float-offset: -10px; }");
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules(&guard);
        let authored = rules
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(rule) => Some(rule.read_with(&guard)),
                _ => None,
            })
            .find_map(|style| {
                style
                    .block
                    .read_with(&guard)
                    .declaration_importance_iter()
                    .find_map(|(declaration, _)| match declaration {
                        PropertyDeclaration::FloatOffset(value) => Some(value.to_css_string()),
                        _ => None,
                    })
            })
            .expect("negative float-offset must enter the typed declaration block");
        assert_eq!(authored, "-10px");

        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new("-10px");
        let mut parser = Parser::new(&mut input);
        let specified =
            crate::values::specified::box_::FloatOffset::parse(&parser_context, &mut parser)
                .expect("signed float-offset grammar is length-percentage");
        parser
            .expect_exhausted()
            .expect("offset consumes its value");

        let stylist = test_stylist();
        let computed = crate::values::computed::Context::for_media_query_evaluation(
            stylist.device(),
            QuirksMode::NoQuirks,
            |context| specified.to_computed_value(context),
        );
        let zero_basis = crate::values::computed::FiniteLength::new_censored(
            crate::values::computed::Length::new(0.0),
        );
        assert_eq!(computed.resolve_finite(zero_basis).px(), -10.0);
    }

    #[test]
    fn servo_censors_float_offset_and_font_size_before_geometry_consumers() {
        use crate::values::computed::length::MAX_FINITE_CSS_LENGTH_PX;

        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let stylist = test_stylist();

        let mut offset_input = ParserInput::new("calc(infinity * 1px + 25%)");
        let mut offset_parser = Parser::new(&mut offset_input);
        let specified_offset =
            crate::values::specified::box_::FloatOffset::parse(&parser_context, &mut offset_parser)
                .expect("mixed non-finite float-offset is valid CSS Values 4 syntax");
        let computed_offset = crate::values::computed::Context::for_media_query_evaluation(
            stylist.device(),
            QuirksMode::NoQuirks,
            |context| specified_offset.to_computed_value(context),
        );
        for basis in [0.0, 100.0] {
            let endpoint = computed_offset.resolve_finite(
                crate::values::computed::FiniteLength::new_censored(
                    crate::values::computed::Length::new(basis),
                ),
            );
            assert_eq!(endpoint.px(), MAX_FINITE_CSS_LENGTH_PX);
            assert!(endpoint.px().is_finite());
        }

        for (css, expected) in [
            ("calc(infinity * 1px)", MAX_FINITE_CSS_LENGTH_PX),
            ("calc(-infinity * 1px)", 0.0),
            ("calc(NaN * 1px)", 0.0),
        ] {
            let mut font_input = ParserInput::new(css);
            let mut font_parser = Parser::new(&mut font_input);
            let specified_font =
                crate::values::specified::FontSize::parse(&parser_context, &mut font_parser)
                    .expect("CSS Values 4 math constants are valid font-size lengths");
            let computed_font = crate::values::computed::Context::for_media_query_evaluation(
                stylist.device(),
                QuirksMode::NoQuirks,
                |context| specified_font.to_computed_value(context),
            );
            let size = computed_font.finite_computed_size().px();
            assert_eq!(size, expected, "computed font size for `{css}`");
            assert!(size.is_finite());
            if expected == 0.0 {
                assert!(size.is_sign_positive());
            }
        }
    }

    #[test]
    fn servo_float_offset_preserves_nonlinear_percentage_basis_semantics() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let stylist = test_stylist();
        let cases: [(&str, &[(f32, f32)]); 3] = [
            ("min(10%, 5px)", &[(20.0, 2.0), (100.0, 5.0)]),
            ("max(10%, 5px)", &[(20.0, 5.0), (100.0, 10.0)]),
            (
                "clamp(5px, 10%, 20px)",
                &[(20.0, 5.0), (100.0, 10.0), (300.0, 20.0)],
            ),
        ];

        for (css, resolutions) in cases {
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            let specified =
                crate::values::specified::box_::FloatOffset::parse(&parser_context, &mut parser)
                    .expect("nonlinear length-percentage math is valid for float-offset");
            let computed = crate::values::computed::Context::for_media_query_evaluation(
                stylist.device(),
                QuirksMode::NoQuirks,
                |context| specified.to_computed_value(context),
            );
            for &(basis, expected) in resolutions {
                let basis = crate::values::computed::FiniteLength::new_censored(
                    crate::values::computed::Length::new(basis),
                );
                assert_eq!(
                    computed.resolve_finite(basis).px(),
                    expected,
                    "`{css}` must retain its exact semantics at a {basis:?} basis",
                );
            }
        }
    }

    #[test]
    fn servo_float_offset_closed_fold_preserves_operator_and_nonfinite_semantics() {
        use crate::values::computed::box_::{
            FloatOffsetCalculationFold, FloatOffsetCalculationScalar,
        };

        #[derive(Debug)]
        enum Folded {
            Length(FloatOffsetCalculationScalar),
            Percentage(FloatOffsetCalculationScalar),
            Number(FloatOffsetCalculationScalar),
            Negate(Box<Self>),
            Invert(Box<Self>),
            Sum(Vec<Self>),
            Product(Vec<Self>),
            Min(Vec<Self>),
            Max(Vec<Self>),
            Clamp(Box<Self>, Box<Self>, Box<Self>),
        }

        impl Folded {
            fn scalar(value: FloatOffsetCalculationScalar) -> f32 {
                match value {
                    FloatOffsetCalculationScalar::Finite(value) => value.get(),
                    FloatOffsetCalculationScalar::PositiveInfinity => f32::INFINITY,
                    FloatOffsetCalculationScalar::NegativeInfinity => f32::NEG_INFINITY,
                    FloatOffsetCalculationScalar::NaN => f32::NAN,
                }
            }

            fn resolve(&self, basis: f32) -> f32 {
                match self {
                    Self::Length(value) | Self::Number(value) => Self::scalar(*value),
                    Self::Percentage(value) => basis * Self::scalar(*value),
                    Self::Negate(value) => -value.resolve(basis),
                    Self::Invert(value) => 1.0 / value.resolve(basis),
                    Self::Sum(values) => values.iter().map(|value| value.resolve(basis)).sum(),
                    Self::Product(values) => {
                        values.iter().map(|value| value.resolve(basis)).product()
                    },
                    Self::Min(values) => values
                        .iter()
                        .map(|value| value.resolve(basis))
                        .fold(f32::INFINITY, f32::min),
                    Self::Max(values) => values
                        .iter()
                        .map(|value| value.resolve(basis))
                        .fold(f32::NEG_INFINITY, f32::max),
                    Self::Clamp(min, center, max) => center
                        .resolve(basis)
                        .max(min.resolve(basis))
                        .min(max.resolve(basis)),
                }
            }

            fn contains_scalar(&self, expected: FloatOffsetCalculationScalar) -> bool {
                match self {
                    Self::Length(value) | Self::Percentage(value) | Self::Number(value) => {
                        *value == expected
                    },
                    Self::Negate(value) | Self::Invert(value) => value.contains_scalar(expected),
                    Self::Sum(values)
                    | Self::Product(values)
                    | Self::Min(values)
                    | Self::Max(values) => {
                        values.iter().any(|value| value.contains_scalar(expected))
                    },
                    Self::Clamp(min, center, max) => {
                        min.contains_scalar(expected)
                            || center.contains_scalar(expected)
                            || max.contains_scalar(expected)
                    },
                }
            }
        }

        struct Fold;

        impl FloatOffsetCalculationFold for Fold {
            type Output = Folded;

            fn length(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output {
                Folded::Length(value)
            }
            fn percentage(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output {
                Folded::Percentage(value)
            }
            fn number(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output {
                Folded::Number(value)
            }
            fn negate(&mut self, value: Self::Output) -> Self::Output {
                Folded::Negate(Box::new(value))
            }
            fn invert(&mut self, value: Self::Output) -> Self::Output {
                Folded::Invert(Box::new(value))
            }
            fn sum(&mut self, values: Vec<Self::Output>) -> Self::Output {
                Folded::Sum(values)
            }
            fn product(&mut self, values: Vec<Self::Output>) -> Self::Output {
                Folded::Product(values)
            }
            fn min(&mut self, values: Vec<Self::Output>) -> Self::Output {
                Folded::Min(values)
            }
            fn max(&mut self, values: Vec<Self::Output>) -> Self::Output {
                Folded::Max(values)
            }
            fn clamp(
                &mut self,
                min: Self::Output,
                center: Self::Output,
                max: Self::Output,
            ) -> Self::Output {
                Folded::Clamp(Box::new(min), Box::new(center), Box::new(max))
            }
            fn round_nearest(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("round() is outside this fold-dispatch test")
            }
            fn round_up(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("round() is outside this fold-dispatch test")
            }
            fn round_down(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("round() is outside this fold-dispatch test")
            }
            fn round_to_zero(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("round() is outside this fold-dispatch test")
            }
            fn modulo(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("mod() is outside this fold-dispatch test")
            }
            fn remainder(&mut self, _: Self::Output, _: Self::Output) -> Self::Output {
                unreachable!("rem() is outside this fold-dispatch test")
            }
            fn hypot(&mut self, _: Vec<Self::Output>) -> Self::Output {
                unreachable!("hypot() is outside this fold-dispatch test")
            }
            fn abs(&mut self, _: Self::Output) -> Self::Output {
                unreachable!("abs() is outside this fold-dispatch test")
            }
            fn sign(&mut self, _: Self::Output) -> Self::Output {
                unreachable!("sign() is outside this fold-dispatch test")
            }
            fn progress_clamped(
                &mut self,
                _: Self::Output,
                _: Self::Output,
                _: Self::Output,
            ) -> Self::Output {
                unreachable!("progress() is outside this fold-dispatch test")
            }
            fn progress_unclamped(
                &mut self,
                _: Self::Output,
                _: Self::Output,
                _: Self::Output,
            ) -> Self::Output {
                unreachable!("progress() is outside this fold-dispatch test")
            }
        }

        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let stylist = test_stylist();
        let compute = |css: &str| {
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            let specified =
                crate::values::specified::box_::FloatOffset::parse(&parser_context, &mut parser)
                    .expect("test float-offset must parse");
            crate::values::computed::Context::for_media_query_evaluation(
                stylist.device(),
                QuirksMode::NoQuirks,
                |context| specified.to_computed_value(context),
            )
        };

        #[derive(Clone, Copy)]
        enum ExpectedRoot {
            Min,
            Max,
            Clamp,
        }

        for (css, expected_root, resolutions) in [
            (
                "min(10%, 5px)",
                ExpectedRoot::Min,
                &[(20.0, 2.0), (100.0, 5.0)][..],
            ),
            (
                "max(10%, 5px)",
                ExpectedRoot::Max,
                &[(20.0, 5.0), (100.0, 10.0)][..],
            ),
            (
                "clamp(5px, 10%, 20px)",
                ExpectedRoot::Clamp,
                &[(20.0, 5.0), (100.0, 10.0), (300.0, 20.0)][..],
            ),
        ] {
            let folded = compute(css).fold_calculation(&mut Fold).unwrap();
            assert!(
                matches!(
                    (expected_root, &folded),
                    (ExpectedRoot::Min, Folded::Min(_))
                        | (ExpectedRoot::Max, Folded::Max(_))
                        | (ExpectedRoot::Clamp, Folded::Clamp(..))
                ),
                "`{css}` dispatched to the wrong callback: {folded:?}",
            );
            for &(basis, expected) in resolutions {
                assert_eq!(
                    folded.resolve(basis),
                    expected,
                    "folded `{css}` at {basis}px"
                );
            }
        }

        for (css, semantic, censored) in [
            (
                "calc(infinity * 1px)",
                FloatOffsetCalculationScalar::PositiveInfinity,
                crate::values::computed::length::MAX_FINITE_CSS_LENGTH_PX,
            ),
            (
                "calc(-infinity * 1px)",
                FloatOffsetCalculationScalar::NegativeInfinity,
                -crate::values::computed::length::MAX_FINITE_CSS_LENGTH_PX,
            ),
            ("calc(NaN * 1px)", FloatOffsetCalculationScalar::NaN, 0.0),
        ] {
            let computed = compute(css);
            let folded = computed.fold_calculation(&mut Fold).unwrap();
            assert!(
                folded.contains_scalar(semantic),
                "`{css}` lost {semantic:?}: {folded:?}"
            );
            let zero = crate::values::computed::FiniteLength::new_censored(Length::new(0.0));
            assert_eq!(computed.resolve_finite(zero).px(), censored);
        }

        for css in ["anchor(--target top)", "anchor-size(--target width)"] {
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            assert!(
                crate::values::specified::box_::FloatOffset::parse(&parser_context, &mut parser)
                    .is_err(),
                "`{css}` must not bypass the float-offset grammar boundary",
            );
        }
    }

    #[test]
    fn servo_line_clamp_is_only_a_three_longhand_expansion() {
        let stylesheet = parse_stylesheet(r#".test { line-clamp: 3 "CUSTOM"; }"#);
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
        let block = style.block.read_with(&guard);
        let mut expansion = block
            .declaration_importance_iter()
            .filter_map(|(declaration, _)| match declaration {
                PropertyDeclaration::MaxLines(value) => Some(("max-lines", value.to_css_string())),
                PropertyDeclaration::Continue(value) => Some(("continue", value.to_css_string())),
                PropertyDeclaration::BlockEllipsis(value) => {
                    Some(("block-ellipsis", value.to_css_string()))
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        expansion.sort_unstable();

        assert_eq!(
            expansion,
            [
                ("block-ellipsis", r#""CUSTOM""#.to_owned()),
                ("continue", "discard".to_owned()),
                ("max-lines", "3".to_owned()),
            ]
        );
        assert_eq!(
            block.len(),
            3,
            "no independent line-clamp declaration may survive"
        );
    }

    #[test]
    fn servo_line_limits_compute_to_private_positive_counts() {
        fn parse_max_lines(css: &str) -> Result<crate::values::specified::MaxLines, ()> {
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
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            parser
                .parse_entirely(|input| crate::values::specified::MaxLines::parse(&context, input))
                .map_err(|_| ())
        }

        fn parse_legacy_line_clamp(
            css: &str,
        ) -> Result<crate::values::specified::box_::LineClamp, ()> {
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
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            parser
                .parse_entirely(|input| {
                    crate::values::specified::box_::LineClamp::parse(&context, input)
                })
                .map_err(|_| ())
        }

        for invalid in ["0", "-1"] {
            assert!(parse_max_lines(invalid).is_err());
            assert!(parse_legacy_line_clamp(invalid).is_err());
        }

        let stylist = test_stylist();
        let (standard, legacy, legacy_none) =
            crate::values::computed::Context::for_media_query_evaluation(
                stylist.device(),
                QuirksMode::NoQuirks,
                |context| {
                    (
                        parse_max_lines("7")
                            .expect("positive max-lines parses")
                            .to_computed_value(context),
                        parse_legacy_line_clamp("5")
                            .expect("positive -webkit-line-clamp parses")
                            .to_computed_value(context),
                        parse_legacy_line_clamp("none")
                            .expect("legacy none parses")
                            .to_computed_value(context),
                    )
                },
            );

        let crate::values::computed::MaxLines::Lines(standard_count) = standard else {
            panic!("positive max-lines must compute to Lines")
        };
        let standard_count: std::num::NonZeroU32 = standard_count.get();
        assert_eq!(standard_count.get(), 7);

        let legacy_count: std::num::NonZeroU32 = legacy
            .lines()
            .expect("positive legacy clamp has lines")
            .get();
        assert_eq!(legacy_count.get(), 5);
        assert!(legacy_none.lines().is_none());
    }

    #[test]
    fn servo_line_clamp_globals_and_variables_expand_atomically() {
        for keyword in ["initial", "inherit", "unset", "revert", "revert-layer"] {
            let stylesheet = parse_stylesheet(&format!(".test {{ line-clamp: {keyword}; }}"));
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
            let block = style.block.read_with(&guard);
            let mut expansion = block
                .declaration_importance_iter()
                .filter_map(|(declaration, _)| {
                    declaration
                        .get_css_wide_keyword()
                        .map(|wide| (declaration.id().to_css_string(), wide.to_str()))
                })
                .collect::<Vec<_>>();
            expansion.sort_unstable();
            assert_eq!(
                expansion,
                [
                    ("block-ellipsis".to_owned(), keyword),
                    ("continue".to_owned(), keyword),
                    ("max-lines".to_owned(), keyword),
                ]
            );
            assert_eq!(block.len(), 3);
        }

        let stylesheet = parse_stylesheet(".test { line-clamp: var(--clamp); }");
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
        let block = style.block.read_with(&guard);
        let mut ids = block
            .declaration_importance_iter()
            .filter_map(|(declaration, _)| {
                matches!(declaration, PropertyDeclaration::WithVariables(..))
                    .then(|| declaration.id().to_css_string())
            })
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, ["block-ellipsis", "continue", "max-lines"]);
        assert_eq!(block.len(), 3);
    }

    #[test]
    fn servo_line_clamp_longhands_have_their_specified_inheritance() {
        assert!(LonghandId::BlockEllipsis.inherited());
        assert!(!LonghandId::MaxLines.inherited());
        assert!(!LonghandId::Continue.inherited());
    }

    #[test]
    fn text_box_trim_and_edge_have_their_specified_inheritance() {
        assert!(!LonghandId::LeadingTrim.inherited());
        assert!(LonghandId::TextBoxEdge.inherited());
    }

    #[test]
    fn text_box_shorthand_expands_trim_and_edge_longhands() {
        let stylesheet = parse_stylesheet(".test { text-box: trim-both cap alphabetic; }");
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
        let block = style.block.read_with(&guard);
        let mut expanded = block
            .declaration_importance_iter()
            .filter_map(|(declaration, _)| match declaration {
                PropertyDeclaration::LeadingTrim(value) => {
                    Some(("leading-trim", value.to_css_string()))
                },
                PropertyDeclaration::TextBoxEdge(value) => {
                    Some(("text-box-edge", value.to_css_string()))
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        expanded.sort_unstable();

        assert_eq!(
            expanded,
            [
                ("leading-trim", "both".to_owned()),
                ("text-box-edge", "cap alphabetic".to_owned())
            ],
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
                        PropertyDeclaration::DominantBaseline(value) => Some(value.to_css_string()),
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
    fn servo_parses_transform_properties_in_margin_box() {
        let stylesheet = parse_stylesheet(
            "@page { @top-left-corner { transform: translate(20px, 30px) rotate(15deg); transform-origin: left top; transform-box: border-box; } }",
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
            3,
            "the transform, transform-origin, and transform-box longhands should parse in a margin box",
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
    fn servo_rejects_multivalue_standard_bleed_at_the_parser_boundary() {
        for invalid in ["1pt 2pt", "1pt 2pt 3pt", "1pt 2pt 3pt 4pt"] {
            let stylesheet = parse_stylesheet(&format!("@page {{ bleed: {invalid}; }}"));
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
                !page_rule_css.contains("bleed:"),
                "invalid standard bleed must not enter the declaration block: {page_rule_css}",
            );
        }
    }

    #[test]
    fn servo_parses_typed_prince_bleed_grammar() {
        for value in [
            "auto",
            "1pt",
            "1pt 2pt",
            "1pt 2pt 3pt",
            "1pt 2pt 3pt 4pt",
            "calc(2pt + max(1em, 3pt))",
            "var(--prince-bleed)",
        ] {
            let stylesheet = parse_stylesheet(&format!("@page {{ -bd-prince-bleed: {value}; }}"));
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
                page_rule_css.contains("-bd-prince-bleed:"),
                "valid Prince bleed must enter the typed declaration block: {page_rule_css}",
            );
        }
    }

    #[test]
    fn servo_rejects_invalid_prince_bleed_at_the_parser_boundary() {
        for invalid in ["none", "auto 1pt", "1pt 2pt 3pt 4pt 5pt"] {
            let stylesheet = parse_stylesheet(&format!("@page {{ -bd-prince-bleed: {invalid}; }}"));
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
                !page_rule_css.contains("-bd-prince-bleed:"),
                "invalid Prince bleed must not enter the typed declaration block: {page_rule_css}",
            );
        }
    }

    #[test]
    fn page_and_bleed_relative_units_use_distinct_device_bases() {
        use crate::values::specified::length::PageRelativeLength;

        let mut stylist = test_stylist();
        stylist
            .device_mut()
            .set_page_box_size(Size2D::<f32, CSSPixel>::new(200.0, 300.0));
        stylist
            .device_mut()
            .set_bleed_box_size(Size2D::<f32, CSSPixel>::new(240.0, 360.0));
        let (pw, ph, bw, bh) = crate::values::computed::Context::for_media_query_evaluation(
            stylist.device(),
            QuirksMode::NoQuirks,
            |context| {
                (
                    PageRelativeLength::Pw(1.0).to_computed_value(context),
                    PageRelativeLength::Ph(1.0).to_computed_value(context),
                    PageRelativeLength::Bw(1.0).to_computed_value(context),
                    PageRelativeLength::Bh(1.0).to_computed_value(context),
                )
            },
        );
        assert_eq!((pw.px(), ph.px()), (2.0, 3.0));
        assert_eq!((bw.px(), bh.px()), (2.4, 3.6));
    }

    #[test]
    fn invalid_later_multivalue_bleed_cannot_erase_an_earlier_scalar() {
        for css in [
            "@page { bleed: 5pt; bleed: 1pt 2pt; }",
            "@page { bleed: 5pt; bleed: 1pt 2pt !important; }",
        ] {
            let stylesheet = parse_stylesheet(css);
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
                page_rule_css.contains("bleed: 5pt"),
                "the valid scalar must survive the invalid later declaration: {page_rule_css}",
            );
            assert!(
                !page_rule_css.contains("1pt 2pt"),
                "the invalid multivalue declaration must never enter the typed block: {page_rule_css}",
            );
        }
    }

    #[test]
    fn negative_bleed_remains_signed_in_the_computed_value() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new("-3px");
        let mut parser = Parser::new(&mut input);
        let specified = crate::values::specified::page::Bleed::parse(&parser_context, &mut parser)
            .expect("negative standard bleed is valid");
        parser
            .expect_exhausted()
            .expect("the scalar consumes the whole value");

        let stylist = test_stylist();
        let computed = crate::values::computed::Context::for_media_query_evaluation(
            stylist.device(),
            QuirksMode::NoQuirks,
            |context| specified.to_computed_value(context),
        );
        match computed {
            crate::values::computed::page::Bleed::Length(length) => {
                assert_eq!(
                    length.px(),
                    -3.0,
                    "computed bleed must preserve its valid sign"
                );
            },
            crate::values::computed::page::Bleed::Auto => panic!("expected computed signed length"),
        }
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
    fn assert_bd_roundtrip(css: &str, property_name: &str, expected_value: &str) {
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

    /// Parse a declaration and assert that the value is rejected rather than
    /// retained as an untyped fork extension.
    fn assert_bd_rejected(css: &str) {
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
            0,
            "expected invalid declaration to be rejected in `{css}`"
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
            "p { -bd-footnote-fragmentation: avoid; }",
            "-bd-footnote-fragmentation",
            "avoid",
        );
    }

    #[test]
    fn servo_preserves_bd_footnote_fragmentation_repeat_declaration() {
        assert_bd_roundtrip(
            "p { -bd-footnote-fragmentation: repeat; }",
            "-bd-footnote-fragmentation",
            "repeat",
        );
    }

    #[test]
    fn servo_preserves_bd_footnote_fragmentation_break_declaration() {
        assert_bd_roundtrip(
            "p { -bd-footnote-fragmentation: break; }",
            "-bd-footnote-fragmentation",
            "break",
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
    fn servo_preserves_bd_sidenote_side_declaration() {
        assert_bd_roundtrip(
            "p { -bd-sidenote-side: outside; }",
            "-bd-sidenote-side",
            "outside",
        );
    }

    #[test]
    fn servo_preserves_bd_sidenote_align_container_start_strict_declaration() {
        assert_bd_roundtrip(
            "p { -bd-sidenote-align: container-start strict; }",
            "-bd-sidenote-align",
            "container-start strict",
        );
    }

    #[test]
    fn servo_rejects_strict_for_unanchored_bd_sidenote_alignments() {
        for value in ["start strict", "end strict", "stack strict"] {
            assert_bd_rejected(&format!("p {{ -bd-sidenote-align: {value}; }}"));
        }
    }

    #[test]
    fn servo_sidenote_controls_are_not_inherited() {
        for property in [
            LonghandId::BdSidenoteAlign,
            LonghandId::BdSidenoteSide,
            LonghandId::BdSidenoteAvoid,
            LonghandId::BdSidenoteOffset,
        ] {
            assert!(
                !property.inherited(),
                "{} must reset to its initial value on descendants",
                property.name()
            );
        }
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
        assert_bd_roundtrip("p { -bd-line-grid: create; }", "-bd-line-grid", "create");
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
            "p { -bd-pdf-attachment-location: document; }",
            "-bd-pdf-attachment-location",
            "document",
        );
        assert_bd_roundtrip(
            "p { -bd-pdf-attachment-order: after; }",
            "-bd-pdf-attachment-order",
            "after",
        );
    }

    // ----- F10 ---------------------------------------------------------
    #[test]
    fn servo_preserves_bookmark_target_counter_declaration() {
        assert_bd_roundtrip("p { bookmark-target: 3; }", "bookmark-target", "3");
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
        assert_bd_roundtrip("p { -bd-link: none; }", "-bd-link", "none");
    }

    #[test]
    fn servo_preserves_bd_link_area_declaration() {
        assert_bd_roundtrip("p { -bd-link-area: text; }", "-bd-link-area", "text");
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

    // ----- F15 ---------------------------------------------------------
    #[test]
    fn servo_barcode_colour_is_non_inherited_with_black_initial() {
        assert!(
            !LonghandId::BdBarcodeColour.inherited(),
            "-bd-barcode-colour must reset on descendants"
        );

        let initial = ComputedValues::initial_values_with_font_override(Font::initial_values());
        assert_eq!(
            initial.get_counters()._bd_barcode_colour.to_css_string(),
            "rgb(0, 0, 0)"
        );
    }

    #[test]
    fn servo_barcode_colour_accepts_css_colours_but_not_auto() {
        assert_bd_roundtrip(
            "p { -bd-barcode-colour: currentColor; }",
            "-bd-barcode-colour",
            "currentcolor",
        );
        assert_bd_roundtrip(
            "p { -bd-barcode-colour: #c02030; }",
            "-bd-barcode-colour",
            "rgb(192, 32, 48)",
        );
        assert_bd_rejected("p { -bd-barcode-colour: auto; }");
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
        assert_bd_roundtrip("p { -bd-text-wrap: balance; }", "-bd-text-wrap", "balance");
    }

    #[test]
    fn servo_preserves_bd_n_lines_declaration() {
        assert_bd_roundtrip("p { -bd-n-lines: 5; }", "-bd-n-lines", "5");
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
                -bd-scale-content: 75%;
                -bd-position-origin: padding;
                -bd-line-break-opportunity: normal '' '/';
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

    #[test]
    fn servo_parses_scrollbar_properties() {
        assert_standard_properties(&["scrollbar-width", "scrollbar-color", "scrollbar-gutter"]);
    }

    #[test]
    fn servo_parses_scroll_margin_and_padding_families() {
        assert_standard_properties(&[
            "scroll-margin",
            "scroll-margin-block",
            "scroll-margin-inline",
            "scroll-padding",
            "scroll-padding-block",
            "scroll-padding-inline",
        ]);

        assert_parsed_declaration_count(
            ".target { \
             scroll-margin: 1px 2px 3px 4px; \
             scroll-margin-block: 5px 6px; \
             scroll-margin-inline: 7px 8px; \
             scroll-padding: auto 10% 11px 12%; \
             scroll-padding-block: 13px 14%; \
             scroll-padding-inline: auto 15px; \
             }",
            16,
        );
    }

    #[test]
    fn servo_parses_scroll_interaction_properties() {
        assert_standard_properties(&[
            "overflow-anchor",
            "overscroll-behavior",
            "overscroll-behavior-block",
            "overscroll-behavior-inline",
            "scroll-behavior",
            "scroll-snap-align",
            "scroll-snap-stop",
            "scroll-snap-type",
            "touch-action",
        ]);

        assert_parsed_declaration_count(
            ".target { \
             overflow-anchor: none; \
             overscroll-behavior: contain none; \
             overscroll-behavior-block: auto; \
             overscroll-behavior-inline: contain; \
             scroll-behavior: smooth; \
             scroll-snap-align: end start; \
             scroll-snap-stop: always; \
             scroll-snap-type: inline mandatory; \
             touch-action: pan-x pan-down pinch-zoom; \
             }",
            10,
        );
    }

    #[test]
    fn servo_parses_standard_ui_colour_and_selection_properties() {
        assert_standard_properties(&["accent-color", "user-select"]);

        assert_parsed_declaration_count(
            ".target { \
             accent-color: rebeccapurple; \
             user-select: contain; \
             }",
            2,
        );
    }

    #[test]
    fn servo_parses_svg_pointer_event_values() {
        for value in [
            "bounding-box",
            "visiblepainted",
            "visiblefill",
            "visiblestroke",
            "visible",
            "painted",
            "fill",
            "stroke",
            "all",
        ] {
            assert_parsed_declaration_count(&format!(".target {{ pointer-events: {value}; }}"), 1);
        }
    }

    #[test]
    fn servo_parses_standard_fill_color() {
        assert_standard_properties(&["fill-color"]);
        for value in ["currentcolor", "rebeccapurple", "transparent"] {
            assert_parsed_declaration_count(&format!(".target {{ fill-color: {value}; }}"), 1);
        }
    }

    #[test]
    fn servo_parses_standard_speech_participation() {
        assert_standard_properties(&["speak"]);
        for value in ["auto", "never", "always"] {
            assert_parsed_declaration_count(&format!(".target {{ speak: {value}; }}"), 1);
        }
    }

    #[test]
    fn servo_parses_level_four_white_space_keywords() {
        for declaration in [
            "white-space-collapse: discard",
            "white-space-collapse: preserve-spaces",
            "text-wrap-style: pretty",
        ] {
            assert_parsed_declaration_count(&format!(".target {{ {declaration}; }}"), 1);
        }
    }

    #[test]
    fn servo_parses_standard_text_size_adjust_values() {
        assert_standard_properties(&["text-size-adjust"]);
        assert_parsed_declaration_count(".target { text-size-adjust: calc(0% + 0%); }", 1);
        assert_parsed_declaration_count(".target { text-size-adjust: -1%; }", 0);
    }

    #[test]
    fn servo_exposes_text_underline_position() {
        assert!(
            crate::properties::PropertyId::parse_enabled_for_all_content("text-underline-position")
                .is_ok(),
            "the standard inherited discrete property must enter the Servo parser",
        );
    }

    #[test]
    fn servo_parses_mask_geometry_boxes() {
        let stylesheet = parse_stylesheet(
            ".a { mask-clip: fill-box } .b { mask-clip: stroke-box } \
             .c { mask-clip: view-box } .d { mask-clip: no-clip } \
             .e { mask-origin: fill-box } .f { mask-origin: stroke-box } \
             .g { mask-origin: view-box }",
        );
        let guard = stylesheet.shared_lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let declaration_count = contents
            .rules(&guard)
            .iter()
            .filter_map(|rule| match rule {
                CssRule::Style(style) => {
                    Some(style.read_with(&guard).block.read_with(&guard).len())
                },
                _ => None,
            })
            .sum::<usize>();

        assert_eq!(declaration_count, 7);
    }

    /// Fragmentation containers pass their leading-margin policy to nested
    /// fragmentation content through the normal inherited cascade.
    #[test]
    fn servo_bd_truncate_margin_after_break_is_inherited() {
        assert!(
            LonghandId::BdTruncateMarginAfterBreak.inherited(),
            "-bd-truncate-margin-after-break must inherit into fragmentation containers"
        );
    }

    /// moegoe F21 — gap fillers: mask-border-* family, the native
    /// border-clip extension, overlay, and native CJK justification parse.
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
                -bd-border-clip: square;
                overlay: auto;
                text-justify: -bd-cjk;
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

        let stylesheet = parse_stylesheet(r#"p::after { content: -bd-attr(data-label); }"#);
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

        let stylesheet =
            parse_stylesheet(r#"span::before { content: -bd-attr-ancestor(data-section); }"#);
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
        let serialised = bd_spot_color_declaration("p { color: -bd-spot(PANTONE-185, 0.5); }")
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
        let stylesheet = parse_stylesheet("@page :first-of-group { margin-top: 5cm; }");
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
            pseudos.iter().any(|pc| matches!(
                pc,
                crate::stylesheets::page_rule::PagePseudoClass::FirstOfGroup
            )),
            "selector should carry the :first-of-group page pseudo"
        );
    }
}
