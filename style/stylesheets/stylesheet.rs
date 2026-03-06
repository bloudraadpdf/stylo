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
    use crate::stylesheets::CssRule;
    use servo_arc::Arc;

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
        let stylesheet =
            parse_stylesheet("@page { @top-center { width: 100px; height: 50px; } }");
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
    fn servo_parses_counter_with_lower_roman() {
        let stylesheet = parse_stylesheet(
            r#"@page { @top-center { content: counter(page, lower-roman); } }"#,
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
            "content with counter(page, lower-roman) should parse in margin box"
        );
    }

    #[test]
    fn servo_parses_counter_with_upper_roman() {
        let stylesheet = parse_stylesheet(
            r#"@page { @top-center { content: counter(page, upper-roman); } }"#,
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
        let stylesheet =
            parse_stylesheet("div { break-before: recto; break-after: verso; }");
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
    fn servo_parses_bookmark_level() {
        let stylesheet = parse_stylesheet(
            "h1 { bookmark-level: 1; } h2 { bookmark-level: none; }",
        );
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
        let stylesheet = parse_stylesheet(
            r#"h1 { bookmark-label: "Chapter " counter(chapter); }"#,
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
            1,
            "bookmark-label with string + counter() should parse"
        );
    }

    #[test]
    fn servo_parses_bookmark_state() {
        let stylesheet = parse_stylesheet(
            "h1 { bookmark-state: open; } h2 { bookmark-state: closed; }",
        );
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
}
