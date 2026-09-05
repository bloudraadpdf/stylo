/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Style sheets and their CSS rules.

mod bd_colour_rule;
mod color_profile_rule;
pub mod container_rule;
mod counter_style_rule;
mod document_rule;
mod font_face_rule;
pub mod font_feature_values_rule;
pub mod font_palette_values_rule;
mod footnote_rule;
pub mod import_rule;
pub mod keyframes_rule;
pub mod layer_rule;
mod loader;
mod margin_rule;
mod media_rule;
mod namespace_rule;
mod nested_declarations_rule;
pub mod origin;
mod page_rule;
pub mod position_try_rule;
mod property_rule;
mod region_rule;
mod rule_list;
mod rule_parser;
mod rules_iterator;
pub mod scope_rule;
mod sidenote_rule;
mod starting_style_rule;
mod style_rule;
mod stylesheet;
pub mod supports_rule;
pub mod when_rule;

use crate::derives::*;
#[cfg(feature = "gecko")]
use crate::gecko_bindings::sugar::refptr::RefCounted;
#[cfg(feature = "gecko")]
use crate::gecko_bindings::{bindings, structs};
use crate::parser::{NestingContext, ParserContext};
use crate::properties::{parse_property_declaration_list, PropertyDeclarationBlock};
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use cssparser::{parse_one_rule, Parser, ParserInput};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::borrow::Cow;
use std::fmt::{self, Write};
#[cfg(feature = "gecko")]
use std::mem::{self, ManuallyDrop};
use style_traits::{CssStringWriter, ParsingMode};
use to_shmem::{SharedMemoryBuilder, ToShmem};

pub use self::bd_colour_rule::{
    parse_bd_colour_body, parse_bd_colour_name, BdColourAlternateKind, BdColourRule,
};
pub use self::color_profile_rule::{
    parse_color_profile_body, parse_color_profile_name, ColorProfileRenderingIntent,
    ColorProfileRule,
};
pub use self::container_rule::ContainerRule;
pub use self::counter_style_rule::CounterStyleRule;
pub use self::document_rule::DocumentRule;
pub use self::font_face_rule::FontFaceRule;
pub use self::font_feature_values_rule::FontFeatureValuesRule;
pub use self::font_palette_values_rule::FontPaletteValuesRule;
pub use self::footnote_rule::FootnoteRule;
pub use self::import_rule::ImportRule;
pub use self::keyframes_rule::KeyframesRule;
pub use self::layer_rule::{LayerBlockRule, LayerStatementRule};
pub use self::loader::StylesheetLoader;
pub use self::margin_rule::{MarginRule, MarginRuleType};
pub use self::media_rule::{
    CustomMediaCondition, CustomMediaEvaluator, CustomMediaMap, CustomMediaRule, MediaRule,
};
pub use self::namespace_rule::NamespaceRule;
pub use self::nested_declarations_rule::NestedDeclarationsRule;
pub use self::origin::{Origin, OriginSet, OriginSetIterator, PerOrigin, PerOriginIter};
pub use self::page_rule::{PagePseudoClassFlags, PageRule, PageSelector, PageSelectors};
pub use self::position_try_rule::PositionTryRule;
pub use self::property_rule::PropertyRule;
pub use self::region_rule::RegionRule;
pub use self::rule_list::CssRules;
pub use self::rule_parser::{InsertRuleContext, State, TopLevelRuleParser};
pub use self::rules_iterator::{AllRules, EffectiveRules};
pub use self::rules_iterator::{
    EffectiveRulesIterator, NestedRuleIterationCondition, RulesIterator,
};
pub use self::scope_rule::ScopeRule;
pub use self::sidenote_rule::SidenoteRule;
pub use self::starting_style_rule::StartingStyleRule;
pub use self::style_rule::StyleRule;
pub use self::stylesheet::{AllowImportRules, SanitizationData, SanitizationKind};
pub use self::stylesheet::{DocumentStyleSheet, Namespaces, Stylesheet};
pub use self::stylesheet::{StylesheetContents, StylesheetInDocument, UserAgentStylesheets};
pub use self::supports_rule::SupportsRule;
pub use self::when_rule::{ChainConditions, ElseRule, WhenCondition, WhenRule};

/// The CORS mode used for a CSS load.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize, ToShmem)]
pub enum CorsMode {
    /// No CORS mode, so cross-origin loads can be done.
    None,
    /// Anonymous CORS request.
    Anonymous,
}

/// Extra data that the backend may need to resolve url values.
///
/// If the usize's lowest bit is 0, then this is a strong reference to a
/// structs::URLExtraData object.
///
/// Otherwise, shifting the usize's bits the right by one gives the
/// UserAgentStyleSheetID value corresponding to the style sheet whose
/// URLExtraData this is, which is stored in URLExtraData_sShared.  We don't
/// hold a strong reference to that object from here, but we rely on that
/// array's objects being held alive until shutdown.
///
/// We use this packed representation rather than an enum so that
/// `from_ptr_ref` can work.
#[cfg(feature = "gecko")]
// Although deriving MallocSizeOf means it always returns 0, that is fine because UrlExtraData
// objects are reference-counted.
#[derive(MallocSizeOf, PartialEq)]
#[repr(C)]
pub struct UrlExtraData(usize);

/// Extra data that the backend may need to resolve url values.
#[cfg(feature = "servo")]
#[derive(Clone, Debug, Eq, MallocSizeOf, PartialEq)]
pub struct UrlExtraData(#[ignore_malloc_size_of = "Arc"] pub Arc<::url::Url>);

#[cfg(feature = "servo")]
impl UrlExtraData {
    /// True if this URL scheme is chrome.
    pub fn chrome_rules_enabled(&self) -> bool {
        self.0.scheme() == "chrome"
    }

    /// Get the interior Url as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "servo")]
impl From<::url::Url> for UrlExtraData {
    fn from(url: ::url::Url) -> Self {
        Self(Arc::new(url))
    }
}

#[cfg(not(feature = "gecko"))]
impl ToShmem for UrlExtraData {
    fn to_shmem(&self, _builder: &mut SharedMemoryBuilder) -> to_shmem::Result<Self> {
        unimplemented!("If servo wants to share stylesheets across processes, ToShmem for Url must be implemented");
    }
}

#[cfg(feature = "gecko")]
impl Clone for UrlExtraData {
    fn clone(&self) -> UrlExtraData {
        UrlExtraData::new(self.ptr())
    }
}

#[cfg(feature = "gecko")]
impl Drop for UrlExtraData {
    fn drop(&mut self) {
        // No need to release when we have an index into URLExtraData_sShared.
        if self.0 & 1 == 0 {
            unsafe {
                self.as_ref().release();
            }
        }
    }
}

#[cfg(feature = "gecko")]
impl ToShmem for UrlExtraData {
    fn to_shmem(&self, _builder: &mut SharedMemoryBuilder) -> to_shmem::Result<Self> {
        if self.0 & 1 == 0 {
            let shared_extra_datas = unsafe {
                std::ptr::addr_of!(structs::URLExtraData_sShared)
                    .as_ref()
                    .unwrap()
            };
            let self_ptr = self.as_ref() as *const _ as *mut _;
            let sheet_id = shared_extra_datas
                .iter()
                .position(|r| r.mRawPtr == self_ptr);
            let sheet_id = match sheet_id {
                Some(id) => id,
                None => {
                    return Err(String::from(
                        "ToShmem failed for UrlExtraData: expected sheet's URLExtraData to be in \
                         URLExtraData::sShared",
                    ));
                },
            };
            Ok(ManuallyDrop::new(UrlExtraData((sheet_id << 1) | 1)))
        } else {
            Ok(ManuallyDrop::new(UrlExtraData(self.0)))
        }
    }
}

#[cfg(feature = "gecko")]
impl UrlExtraData {
    /// Create a new UrlExtraData wrapping a pointer to the specified Gecko
    /// URLExtraData object.
    pub fn new(ptr: *mut structs::URLExtraData) -> UrlExtraData {
        unsafe {
            (*ptr).addref();
        }
        UrlExtraData(ptr as usize)
    }

    /// True if this URL scheme is chrome.
    #[inline]
    pub fn chrome_rules_enabled(&self) -> bool {
        self.as_ref().mChromeRulesEnabled
    }

    /// Create a reference to this `UrlExtraData` from a reference to pointer.
    ///
    /// The pointer must be valid and non null.
    ///
    /// This method doesn't touch refcount.
    #[inline]
    pub unsafe fn from_ptr_ref(ptr: &*mut structs::URLExtraData) -> &Self {
        mem::transmute(ptr)
    }

    /// Returns a pointer to the Gecko URLExtraData object.
    pub fn ptr(&self) -> *mut structs::URLExtraData {
        if self.0 & 1 == 0 {
            self.0 as *mut structs::URLExtraData
        } else {
            unsafe {
                let sheet_id = self.0 >> 1;
                structs::URLExtraData_sShared[sheet_id].mRawPtr
            }
        }
    }

    fn as_ref(&self) -> &structs::URLExtraData {
        unsafe { &*(self.ptr() as *const structs::URLExtraData) }
    }
}

#[cfg(feature = "gecko")]
impl fmt::Debug for UrlExtraData {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        macro_rules! define_debug_struct {
            ($struct_name:ident, $gecko_class:ident, $debug_fn:ident) => {
                struct $struct_name(*mut structs::$gecko_class);
                impl fmt::Debug for $struct_name {
                    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        use nsstring::nsCString;
                        let mut spec = nsCString::new();
                        unsafe {
                            bindings::$debug_fn(self.0, &mut spec);
                        }
                        spec.fmt(formatter)
                    }
                }
            };
        }

        define_debug_struct!(DebugURI, nsIURI, Gecko_nsIURI_Debug);
        define_debug_struct!(
            DebugReferrerInfo,
            nsIReferrerInfo,
            Gecko_nsIReferrerInfo_Debug
        );

        formatter
            .debug_struct("URLExtraData")
            .field("chrome_rules_enabled", &self.chrome_rules_enabled())
            .field("base", &DebugURI(self.as_ref().mBaseURI.raw()))
            .field(
                "referrer",
                &DebugReferrerInfo(self.as_ref().mReferrerInfo.raw()),
            )
            .finish()
    }
}

// XXX We probably need to figure out whether we should mark Eq here.
// It is currently marked so because properties::UnparsedValue wants Eq.
#[cfg(feature = "gecko")]
impl Eq for UrlExtraData {}

/// Serialize a page or style rule, starting with the opening brace.
///
/// https://drafts.csswg.org/cssom/#serialize-a-css-rule CSSStyleRule
///
/// This is not properly specified for page-rules, but we will apply the
/// same process.
fn style_or_page_rule_to_css(
    rules: Option<&Arc<Locked<CssRules>>>,
    block: &Locked<PropertyDeclarationBlock>,
    guard: &SharedRwLockReadGuard,
    dest: &mut CssStringWriter,
) -> fmt::Result {
    // Write the opening brace. The caller needs to serialize up to this point.
    dest.write_char('{')?;

    // Step 2
    let declaration_block = block.read_with(guard);
    let has_declarations = !declaration_block.declarations().is_empty();

    // Step 3
    if let Some(ref rules) = rules {
        let rules = rules.read_with(guard);
        // Step 6 (here because it's more convenient)
        if !rules.is_empty() {
            if has_declarations {
                dest.write_str("\n  ")?;
                declaration_block.to_css(dest)?;
            }
            return rules.to_css_block_without_opening(guard, dest);
        }
    }

    // Steps 4 & 5
    if has_declarations {
        dest.write_char(' ')?;
        declaration_block.to_css(dest)?;
    }
    dest.write_str(" }")
}

/// A CSS rule.
///
/// TODO(emilio): Lots of spec links should be around.
#[derive(Clone, Debug, ToShmem)]
#[allow(missing_docs)]
pub enum CssRule {
    Style(Arc<Locked<StyleRule>>),
    // No Charset here, CSSCharsetRule has been removed from CSSOM
    // https://drafts.csswg.org/cssom/#changes-from-5-december-2013
    Namespace(Arc<NamespaceRule>),
    Import(Arc<Locked<ImportRule>>),
    Media(Arc<MediaRule>),
    CustomMedia(Arc<CustomMediaRule>),
    Container(Arc<ContainerRule>),
    FontFace(Arc<Locked<FontFaceRule>>),
    FontFeatureValues(Arc<FontFeatureValuesRule>),
    FontPaletteValues(Arc<FontPaletteValuesRule>),
    CounterStyle(Arc<Locked<CounterStyleRule>>),
    Keyframes(Arc<Locked<KeyframesRule>>),
    Margin(Arc<MarginRule>),
    Footnote(Arc<FootnoteRule>),
    /// moegoe Family 7 — `@-bd-sidenote` nested page rule.
    Sidenote(Arc<SidenoteRule>),
    /// moegoe Family 2 — `@-bd-colour <name> { … }` top-level rule
    /// declaring a named-spot colour for downstream `-bd-spot()` /
    /// `-bd-separation()` references.
    BdColour(Arc<BdColourRule>),
    /// CSS Color 5 §7 — `@color-profile --name { … }` top-level rule
    /// declaring an ICC profile against a `<dashed-ident>` so
    /// downstream `color(<dashed-ident> ...)` references and the
    /// `output-color-model: <dashed-ident>` value resolve against it.
    ColorProfile(Arc<ColorProfileRule>),
    /// moegoe Family 17 — `@region <selector> { … }` top-level rule
    /// (CSS Regions L1 §6.4). Declarations scope to elements matching
    /// the selector when they appear inside a region-chain descendant.
    Region(Arc<RegionRule>),
    Supports(Arc<SupportsRule>),
    /// CSS Conditional 5 §3.1 — `@when <when-condition> { … }`. The
    /// rule contributes its body to the cascade iff its branch is the
    /// active member of its `@when` / `@else` chain. Chain membership
    /// is encoded by a shared `Arc<ChainConditions>` plus the rule's
    /// `chain_position` (always 0 for `@when`).
    When(Arc<WhenRule>),
    /// CSS Conditional 5 §3.2 — `@else [ <when-condition> ]? { … }`.
    /// Chains immediately after a preceding `@when` (or another
    /// `@else`). A trailing `@else` with no condition is the
    /// unconditional fallback branch.
    Else(Arc<ElseRule>),
    Page(Arc<Locked<PageRule>>),
    Property(Arc<PropertyRule>),
    Document(Arc<DocumentRule>),
    LayerBlock(Arc<LayerBlockRule>),
    LayerStatement(Arc<LayerStatementRule>),
    Scope(Arc<ScopeRule>),
    StartingStyle(Arc<StartingStyleRule>),
    PositionTry(Arc<Locked<PositionTryRule>>),
    NestedDeclarations(Arc<Locked<NestedDeclarationsRule>>),
}

impl CssRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        match *self {
            // Not all fields are currently fully measured. Extra measurement
            // may be added later.
            CssRule::Namespace(_) => 0,

            // We don't need to measure ImportRule::stylesheet because we measure
            // it on the C++ side in the child list of the ServoStyleSheet.
            CssRule::Import(_) => 0,

            CssRule::Style(ref lock) => {
                lock.unconditional_shallow_size_of(ops) + lock.read_with(guard).size_of(guard, ops)
            },
            CssRule::Media(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::CustomMedia(ref arc) => {
                // Measurement of other fields might be added later.
                arc.unconditional_shallow_size_of(ops)
            },
            CssRule::Container(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::FontFace(_) => 0,
            CssRule::FontFeatureValues(_) => 0,
            CssRule::FontPaletteValues(_) => 0,
            CssRule::CounterStyle(_) => 0,
            CssRule::Keyframes(_) => 0,
            CssRule::Margin(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Footnote(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Sidenote(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::BdColour(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::ColorProfile(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Region(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Supports(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::When(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Else(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::Page(ref lock) => {
                lock.unconditional_shallow_size_of(ops) + lock.read_with(guard).size_of(guard, ops)
            },
            CssRule::Property(ref rule) => {
                rule.unconditional_shallow_size_of(ops) + rule.size_of(guard, ops)
            },
            CssRule::Document(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            CssRule::StartingStyle(ref arc) => {
                arc.unconditional_shallow_size_of(ops) + arc.size_of(guard, ops)
            },
            // TODO(emilio): Add memory reporting for these rules.
            CssRule::LayerBlock(_) | CssRule::LayerStatement(_) => 0,
            CssRule::Scope(ref rule) => {
                rule.unconditional_shallow_size_of(ops) + rule.size_of(guard, ops)
            },
            CssRule::PositionTry(ref lock) => {
                lock.unconditional_shallow_size_of(ops) + lock.read_with(guard).size_of(guard, ops)
            },
            CssRule::NestedDeclarations(ref lock) => {
                lock.unconditional_shallow_size_of(ops) + lock.read_with(guard).size_of(guard, ops)
            },
        }
    }

    fn is_empty_nested_declarations(&self, guard: &SharedRwLockReadGuard) -> bool {
        match *self {
            CssRule::NestedDeclarations(ref lock) => {
                lock.read_with(guard).block.read_with(guard).is_empty()
            },
            _ => false,
        }
    }
}

// These aliases are required on Gecko side to avoid generating bindings for `Locked`.
/// Alias for a locked style rule.
pub type LockedStyleRule = Locked<StyleRule>;
/// Alias for a locked import rule.
pub type LockedImportRule = Locked<ImportRule>;
/// Alias for a locked font-face rule.
pub type LockedFontFaceRule = Locked<FontFaceRule>;
/// Alias for a locked counter-style rule.
pub type LockedCounterStyleRule = Locked<CounterStyleRule>;
/// Alias for a locked keyframes rule.
pub type LockedKeyframesRule = Locked<KeyframesRule>;
/// Alias for a locked page rule.
pub type LockedPageRule = Locked<PageRule>;
/// Alias for a locked position-try rule.
pub type LockedPositionTryRule = Locked<PositionTryRule>;
/// Alias for a locked nested declarations rule.
pub type LockedNestedDeclarationsRule = Locked<NestedDeclarationsRule>;

/// A CSS rule reference. Should mirror `CssRule`.
#[repr(C)]
#[allow(missing_docs)]
pub enum CssRuleRef<'a> {
    Style(&'a LockedStyleRule),
    Namespace(&'a NamespaceRule),
    Import(&'a LockedImportRule),
    Media(&'a MediaRule),
    CustomMedia(&'a CustomMediaRule),
    Container(&'a ContainerRule),
    FontFace(&'a LockedFontFaceRule),
    FontFeatureValues(&'a FontFeatureValuesRule),
    FontPaletteValues(&'a FontPaletteValuesRule),
    CounterStyle(&'a LockedCounterStyleRule),
    Keyframes(&'a LockedKeyframesRule),
    Margin(&'a MarginRule),
    Footnote(&'a FootnoteRule),
    /// moegoe Family 7 — `@-bd-sidenote` reference.
    Sidenote(&'a SidenoteRule),
    /// moegoe Family 2 — `@-bd-colour <name> { … }` reference.
    BdColour(&'a BdColourRule),
    /// CSS Color 5 §7 — `@color-profile --name { … }` reference.
    ColorProfile(&'a ColorProfileRule),
    /// moegoe Family 17 — `@region <selector> { … }` reference.
    Region(&'a RegionRule),
    Supports(&'a SupportsRule),
    /// CSS Conditional 5 §3.1 — `@when` rule reference.
    When(&'a WhenRule),
    /// CSS Conditional 5 §3.2 — `@else` rule reference.
    Else(&'a ElseRule),
    Page(&'a LockedPageRule),
    Property(&'a PropertyRule),
    Document(&'a DocumentRule),
    LayerBlock(&'a LayerBlockRule),
    LayerStatement(&'a LayerStatementRule),
    Scope(&'a ScopeRule),
    StartingStyle(&'a StartingStyleRule),
    PositionTry(&'a LockedPositionTryRule),
    NestedDeclarations(&'a LockedNestedDeclarationsRule),
}

impl<'a> From<&'a CssRule> for CssRuleRef<'a> {
    fn from(value: &'a CssRule) -> Self {
        match value {
            CssRule::Style(r) => CssRuleRef::Style(r.as_ref()),
            CssRule::Namespace(r) => CssRuleRef::Namespace(r.as_ref()),
            CssRule::Import(r) => CssRuleRef::Import(r.as_ref()),
            CssRule::Media(r) => CssRuleRef::Media(r.as_ref()),
            CssRule::CustomMedia(r) => CssRuleRef::CustomMedia(r.as_ref()),
            CssRule::Container(r) => CssRuleRef::Container(r.as_ref()),
            CssRule::FontFace(r) => CssRuleRef::FontFace(r.as_ref()),
            CssRule::FontFeatureValues(r) => CssRuleRef::FontFeatureValues(r.as_ref()),
            CssRule::FontPaletteValues(r) => CssRuleRef::FontPaletteValues(r.as_ref()),
            CssRule::CounterStyle(r) => CssRuleRef::CounterStyle(r.as_ref()),
            CssRule::Keyframes(r) => CssRuleRef::Keyframes(r.as_ref()),
            CssRule::Margin(r) => CssRuleRef::Margin(r.as_ref()),
            CssRule::Footnote(r) => CssRuleRef::Footnote(r.as_ref()),
            CssRule::Sidenote(r) => CssRuleRef::Sidenote(r.as_ref()),
            CssRule::BdColour(r) => CssRuleRef::BdColour(r.as_ref()),
            CssRule::ColorProfile(r) => CssRuleRef::ColorProfile(r.as_ref()),
            CssRule::Region(r) => CssRuleRef::Region(r.as_ref()),
            CssRule::Supports(r) => CssRuleRef::Supports(r.as_ref()),
            CssRule::When(r) => CssRuleRef::When(r.as_ref()),
            CssRule::Else(r) => CssRuleRef::Else(r.as_ref()),
            CssRule::Page(r) => CssRuleRef::Page(r.as_ref()),
            CssRule::Property(r) => CssRuleRef::Property(r.as_ref()),
            CssRule::Document(r) => CssRuleRef::Document(r.as_ref()),
            CssRule::LayerBlock(r) => CssRuleRef::LayerBlock(r.as_ref()),
            CssRule::LayerStatement(r) => CssRuleRef::LayerStatement(r.as_ref()),
            CssRule::Scope(r) => CssRuleRef::Scope(r.as_ref()),
            CssRule::StartingStyle(r) => CssRuleRef::StartingStyle(r.as_ref()),
            CssRule::PositionTry(r) => CssRuleRef::PositionTry(r.as_ref()),
            CssRule::NestedDeclarations(r) => CssRuleRef::NestedDeclarations(r.as_ref()),
        }
    }
}

/// https://drafts.csswg.org/cssom-1/#dom-cssrule-type
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, FromPrimitive, PartialEq)]
#[repr(u8)]
pub enum CssRuleType {
    // https://drafts.csswg.org/cssom/#the-cssrule-interface
    Style = 1,
    // Charset historically lived at value 2; CSSOM removed
    // CSSCharsetRule (https://drafts.csswg.org/cssom/#changes-from-5-december-2013),
    // so the slot was free. We reuse it for CSS Conditional 5 §3.1
    // `@when`. The numeric ordering does not feed any external IDL
    // and `CssRuleTypes` is a `u32` bitfield — every discriminant
    // must stay strictly less than 32.
    When = 2,
    Import = 3,
    Media = 4,
    FontFace = 5,
    Page = 6,
    // https://drafts.csswg.org/css-animations-1/#interface-cssrule-idl
    Keyframes = 7,
    Keyframe = 8,
    // https://drafts.csswg.org/cssom/#the-cssrule-interface
    Margin = 9,
    Namespace = 10,
    // https://drafts.csswg.org/css-counter-styles-3/#extentions-to-cssrule-interface
    CounterStyle = 11,
    // https://drafts.csswg.org/css-conditional-3/#extentions-to-cssrule-interface
    Supports = 12,
    // https://www.w3.org/TR/2012/WD-css3-conditional-20120911/#extentions-to-cssrule-interface
    Document = 13,
    // https://drafts.csswg.org/css-fonts/#om-fontfeaturevalues
    FontFeatureValues = 14,
    // CSS Conditional 5 §3.2 `@else`. Slot 15 was historically
    // unallocated (Viewport occupied a different range), and we
    // need to keep every discriminant strictly less than 32 so
    // `CssRuleTypes`' `u32` bitfield can address it.
    Else = 15,
    // After viewport, all rules should return 0 from the API, but we still need
    // a constant somewhere.
    LayerBlock = 16,
    LayerStatement = 17,
    Container = 18,
    FontPaletteValues = 19,
    // 20 is an arbitrary number to use for Property.
    Property = 20,
    Scope = 21,
    // https://drafts.csswg.org/css-transitions-2/#the-cssstartingstylerule-interface
    StartingStyle = 22,
    // https://drafts.csswg.org/css-anchor-position-1/#om-position-try
    PositionTry = 23,
    // https://drafts.csswg.org/css-nesting-1/#nested-declarations-rule
    NestedDeclarations = 24,
    CustomMedia = 25,
    Footnote = 26,
    /// moegoe Family 7 — `@-bd-sidenote` rule type. Slots after the
    /// existing fork-private extension at 26.
    Sidenote = 27,
    /// moegoe Family 2 — `@-bd-colour <name> { … }` rule type. Slots
    /// after the existing fork-private extensions at 26 and 27.
    BdColour = 28,
    /// moegoe Family 17 — `@region <selector> { … }` rule type
    /// (CSS Regions L1 §6.4). Slots after the existing fork-private
    /// extensions at 26, 27, and 28.
    Region = 29,
    /// CSS Color 5 §7 — `@color-profile --name { … }` rule type.
    /// Slots after the existing fork-private extensions at 26–29.
    ColorProfile = 30,
}

impl CssRuleType {
    /// Returns a bit that identifies this rule type.
    #[inline]
    pub const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// Set of rule types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CssRuleTypes(u32);

impl From<CssRuleType> for CssRuleTypes {
    fn from(ty: CssRuleType) -> Self {
        Self(ty.bit())
    }
}

impl CssRuleTypes {
    /// Rules where !important declarations are forbidden.
    pub const IMPORTANT_FORBIDDEN: Self =
        Self(CssRuleType::PositionTry.bit() | CssRuleType::Keyframe.bit());

    /// Returns whether the rule is in the current set.
    #[inline]
    pub fn contains(self, ty: CssRuleType) -> bool {
        self.0 & ty.bit() != 0
    }

    /// Returns all the rules specified in the set.
    #[inline]
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Creates a raw CssRuleTypes bitfield.
    #[inline]
    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns whether the rule set is empty.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Inserts a rule type into the set.
    #[inline]
    pub fn insert(&mut self, ty: CssRuleType) {
        self.0 |= ty.bit()
    }

    /// Returns whether any of the types intersect.
    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[allow(missing_docs)]
pub enum RulesMutateError {
    Syntax,
    IndexSize,
    HierarchyRequest,
    InvalidState,
}

impl CssRule {
    /// Returns the CSSOM rule type of this rule.
    pub fn rule_type(&self) -> CssRuleType {
        match *self {
            CssRule::Style(_) => CssRuleType::Style,
            CssRule::Import(_) => CssRuleType::Import,
            CssRule::Media(_) => CssRuleType::Media,
            CssRule::CustomMedia(_) => CssRuleType::CustomMedia,
            CssRule::FontFace(_) => CssRuleType::FontFace,
            CssRule::FontFeatureValues(_) => CssRuleType::FontFeatureValues,
            CssRule::FontPaletteValues(_) => CssRuleType::FontPaletteValues,
            CssRule::CounterStyle(_) => CssRuleType::CounterStyle,
            CssRule::Keyframes(_) => CssRuleType::Keyframes,
            CssRule::Margin(_) => CssRuleType::Margin,
            CssRule::Footnote(_) => CssRuleType::Footnote,
            CssRule::Sidenote(_) => CssRuleType::Sidenote,
            CssRule::BdColour(_) => CssRuleType::BdColour,
            CssRule::ColorProfile(_) => CssRuleType::ColorProfile,
            CssRule::Region(_) => CssRuleType::Region,
            CssRule::Namespace(_) => CssRuleType::Namespace,
            CssRule::Supports(_) => CssRuleType::Supports,
            CssRule::When(_) => CssRuleType::When,
            CssRule::Else(_) => CssRuleType::Else,
            CssRule::Page(_) => CssRuleType::Page,
            CssRule::Property(_) => CssRuleType::Property,
            CssRule::Document(_) => CssRuleType::Document,
            CssRule::LayerBlock(_) => CssRuleType::LayerBlock,
            CssRule::LayerStatement(_) => CssRuleType::LayerStatement,
            CssRule::Container(_) => CssRuleType::Container,
            CssRule::Scope(_) => CssRuleType::Scope,
            CssRule::StartingStyle(_) => CssRuleType::StartingStyle,
            CssRule::PositionTry(_) => CssRuleType::PositionTry,
            CssRule::NestedDeclarations(_) => CssRuleType::NestedDeclarations,
        }
    }

    /// Parse a CSS rule.
    ///
    /// This mostly implements steps 3..7 of https://drafts.csswg.org/cssom/#insert-a-css-rule
    pub fn parse(
        css: &str,
        insert_rule_context: InsertRuleContext,
        parent_stylesheet_contents: &StylesheetContents,
        shared_lock: &SharedRwLock,
        loader: Option<&dyn StylesheetLoader>,
        allow_import_rules: AllowImportRules,
    ) -> Result<Self, RulesMutateError> {
        let url_data = &parent_stylesheet_contents.url_data;
        let namespaces = &parent_stylesheet_contents.namespaces;
        let mut context = ParserContext::new(
            parent_stylesheet_contents.origin,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            parent_stylesheet_contents.quirks_mode,
            Cow::Borrowed(&*namespaces),
            None,
            None,
        );
        // Override the nesting context with existing data.
        context.nesting_context = NestingContext::new(
            insert_rule_context.containing_rule_types,
            insert_rule_context.parse_relative_rule_type,
        );

        let state = if !insert_rule_context.containing_rule_types.is_empty() {
            State::Body
        } else if insert_rule_context.index == 0 {
            State::Start
        } else {
            let index = insert_rule_context.index;
            insert_rule_context.max_rule_state_at_index(index - 1)
        };

        let mut input = ParserInput::new(css);
        let mut input = Parser::new(&mut input);

        // nested rules are in the body state
        let mut parser = TopLevelRuleParser {
            context,
            shared_lock: &shared_lock,
            loader,
            state,
            dom_error: None,
            insert_rule_context: Some(insert_rule_context),
            allow_import_rules,
            declaration_parser_state: Default::default(),
            first_declaration_block: Default::default(),
            wants_first_declaration_block: false,
            error_reporting_state: Default::default(),
            rules: Default::default(),
        };

        if input
            .try_parse(|input| parse_one_rule(input, &mut parser))
            .is_ok()
        {
            return Ok(parser.rules.pop().unwrap());
        }

        let error = parser.dom_error.take().unwrap_or(RulesMutateError::Syntax);
        // If new rule is a syntax error, and nested is set, perform the following substeps:
        if matches!(error, RulesMutateError::Syntax) && parser.can_parse_declarations() {
            let declarations = parse_property_declaration_list(&parser.context, &mut input, &[]);
            if !declarations.is_empty() {
                return Ok(CssRule::NestedDeclarations(Arc::new(
                    parser.shared_lock.wrap(NestedDeclarationsRule {
                        block: Arc::new(parser.shared_lock.wrap(declarations)),
                        source_location: input.current_source_location(),
                    }),
                )));
            }
        }
        Err(error)
    }
}

impl DeepCloneWithLock for CssRule {
    /// Deep clones this CssRule.
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> CssRule {
        match *self {
            CssRule::Namespace(ref arc) => CssRule::Namespace(arc.clone()),
            CssRule::Import(ref arc) => {
                let rule = arc.read_with(guard).deep_clone_with_lock(lock, guard);
                CssRule::Import(Arc::new(lock.wrap(rule)))
            },
            CssRule::Style(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::Style(Arc::new(lock.wrap(rule.deep_clone_with_lock(lock, guard))))
            },
            CssRule::Container(ref arc) => {
                CssRule::Container(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Media(ref arc) => {
                CssRule::Media(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::CustomMedia(ref arc) => {
                CssRule::CustomMedia(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::FontFace(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::FontFace(Arc::new(lock.wrap(rule.clone())))
            },
            CssRule::FontFeatureValues(ref arc) => CssRule::FontFeatureValues(arc.clone()),
            CssRule::FontPaletteValues(ref arc) => CssRule::FontPaletteValues(arc.clone()),
            CssRule::CounterStyle(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::CounterStyle(Arc::new(lock.wrap(rule.clone())))
            },
            CssRule::Keyframes(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::Keyframes(Arc::new(lock.wrap(rule.deep_clone_with_lock(lock, guard))))
            },
            CssRule::Margin(ref arc) => {
                CssRule::Margin(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Footnote(ref arc) => {
                CssRule::Footnote(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Sidenote(ref arc) => {
                CssRule::Sidenote(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::BdColour(ref arc) => {
                CssRule::BdColour(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::ColorProfile(ref arc) => {
                CssRule::ColorProfile(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Region(ref arc) => {
                CssRule::Region(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Supports(ref arc) => {
                CssRule::Supports(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::When(ref arc) => {
                CssRule::When(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Else(ref arc) => {
                CssRule::Else(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Page(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::Page(Arc::new(lock.wrap(rule.deep_clone_with_lock(lock, guard))))
            },
            CssRule::Property(ref arc) => {
                // @property rules are immutable, so we don't need any of the `Locked`
                // shenanigans, actually, and can just share the rule.
                CssRule::Property(arc.clone())
            },
            CssRule::Document(ref arc) => {
                CssRule::Document(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::LayerStatement(ref arc) => CssRule::LayerStatement(arc.clone()),
            CssRule::LayerBlock(ref arc) => {
                CssRule::LayerBlock(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::Scope(ref arc) => {
                CssRule::Scope(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::StartingStyle(ref arc) => {
                CssRule::StartingStyle(Arc::new(arc.deep_clone_with_lock(lock, guard)))
            },
            CssRule::PositionTry(ref arc) => {
                let rule = arc.read_with(guard);
                CssRule::PositionTry(Arc::new(lock.wrap(rule.deep_clone_with_lock(lock, guard))))
            },
            CssRule::NestedDeclarations(ref arc) => {
                let decls = arc.read_with(guard);
                CssRule::NestedDeclarations(Arc::new(
                    lock.wrap(decls.deep_clone_with_lock(lock, guard)),
                ))
            },
        }
    }
}

impl ToCssWithGuard for CssRule {
    // https://drafts.csswg.org/cssom/#serialize-a-css-rule
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        match *self {
            CssRule::Namespace(ref rule) => rule.to_css(guard, dest),
            CssRule::Import(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::Style(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::FontFace(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::FontFeatureValues(ref rule) => rule.to_css(guard, dest),
            CssRule::FontPaletteValues(ref rule) => rule.to_css(guard, dest),
            CssRule::CounterStyle(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::Keyframes(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::Margin(ref rule) => rule.to_css(guard, dest),
            CssRule::Footnote(ref rule) => rule.to_css(guard, dest),
            CssRule::Sidenote(ref rule) => rule.to_css(guard, dest),
            CssRule::BdColour(ref rule) => rule.to_css(guard, dest),
            CssRule::ColorProfile(ref rule) => rule.to_css(guard, dest),
            CssRule::Region(ref rule) => rule.to_css(guard, dest),
            CssRule::Media(ref rule) => rule.to_css(guard, dest),
            CssRule::CustomMedia(ref rule) => rule.to_css(guard, dest),
            CssRule::Supports(ref rule) => rule.to_css(guard, dest),
            CssRule::When(ref rule) => rule.to_css(guard, dest),
            CssRule::Else(ref rule) => rule.to_css(guard, dest),
            CssRule::Page(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::Property(ref rule) => rule.to_css(guard, dest),
            CssRule::Document(ref rule) => rule.to_css(guard, dest),
            CssRule::LayerBlock(ref rule) => rule.to_css(guard, dest),
            CssRule::LayerStatement(ref rule) => rule.to_css(guard, dest),
            CssRule::Container(ref rule) => rule.to_css(guard, dest),
            CssRule::Scope(ref rule) => rule.to_css(guard, dest),
            CssRule::StartingStyle(ref rule) => rule.to_css(guard, dest),
            CssRule::PositionTry(ref lock) => lock.read_with(guard).to_css(guard, dest),
            CssRule::NestedDeclarations(ref lock) => lock.read_with(guard).to_css(guard, dest),
        }
    }
}
