/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Support for [custom properties for cascading variables][custom].
//!
//! [custom]: https://drafts.csswg.org/css-variables/

use crate::Atom;
use crate::applicable_declarations::CascadePriority;
use crate::custom_properties_map::CustomPropertiesMap;
use crate::derives::*;
use crate::dom::AttributeTracker;
use crate::media_queries::Device;
use crate::parser::Parse;
use crate::properties::{
    CSSWideKeyword, CustomDeclaration, CustomDeclarationValue, LonghandId, LonghandIdSet,
    PropertyDeclaration,
};
use crate::properties_and_values::{
    registry::PropertyRegistrationData,
    syntax::{Descriptor, data_type::DependentDataTypes},
    value::{
        AllowComputationallyDependent, ComputedValue as ComputedRegisteredValue,
        SpecifiedValue as SpecifiedRegisteredValue,
    },
};
use crate::rule_tree::CascadeLevel;
use crate::selector_map::{PrecomputedHashMap, PrecomputedHashSet};
use crate::stylesheets::{Namespaces, UrlExtraData};
use crate::stylist::Stylist;
use crate::values::DashedIdent;
use crate::values::computed::{self, ToComputedValue};
use crate::values::generics::calc::SortKey as AttrUnit;
use crate::values::specified::{AttrName, FontRelativeLength};
use cssparser::{
    CowRcStr, Delimiter, Parser, ParserInput, SourcePosition, Token, TokenSerializationType,
};
use selectors::parser::SelectorParseErrorKind;
use servo_arc::Arc;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::fmt::{self, Write};
use std::ops::{Index, IndexMut};
use std::{cmp, num};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};
use thin_vec::ThinVec;

mod conditionals;
mod functions;

pub use self::functions::{
    CallingElement, CustomFunction, ElementCallSite, FunctionBodyItem, FunctionScope,
    document_function_scope,
};

/// CSS env-1 §3 paged-media environment variables, in CSS pixels.
///
/// Carried by [`CssEnvironment`] so that an embedder (moegoe's paginator)
/// can supply per-@page used dimensions before the cascade resolves
/// `env(page-width)` / `env(page-height)` / `env(page-margin-*)` calls
/// embedded in `@page` declarations or `@page` margin-box content.
///
/// `None` for any field means "no override authored at this resolution
/// site"; the env evaluator returns the default `0px` so authored
/// fallbacks (or the property's initial) take effect.
#[derive(Clone, Copy, Debug, Default, MallocSizeOf, PartialEq)]
pub struct PagedMediaEnvOverrides {
    /// `env(page-width)`, in CSS pixels.
    pub page_width: Option<f32>,
    /// `env(page-height)`, in CSS pixels.
    pub page_height: Option<f32>,
    /// `env(page-margin-top)`, in CSS pixels.
    pub page_margin_top: Option<f32>,
    /// `env(page-margin-right)`, in CSS pixels.
    pub page_margin_right: Option<f32>,
    /// `env(page-margin-bottom)`, in CSS pixels.
    pub page_margin_bottom: Option<f32>,
    /// `env(page-margin-left)`, in CSS pixels.
    pub page_margin_left: Option<f32>,
}

/// The environment from which to get `env` function values.
///
/// Carries an interior-mutable override slot for the CSS env-1 §3
/// paged-media variables (`page-width`, `page-height`,
/// `page-margin-*`). Embedders that drive paged-media cascade (moegoe)
/// install per-@page used geometry here before re-resolving env() so
/// authored `env(page-*)` references collapse to real lengths instead
/// of the static `0px` defaults.
#[derive(Debug, Default, MallocSizeOf)]
pub struct CssEnvironment {
    /// CSS env-1 §3 paged-media overrides; `None` means "no override
    /// installed", in which case the static evaluators return `0px`.
    #[ignore_malloc_size_of = "Trivial copy in RwLock"]
    paged_media_overrides: parking_lot::RwLock<Option<PagedMediaEnvOverrides>>,
}

impl CssEnvironment {
    /// Install per-@page paged-media env-var overrides. Pass `None` to
    /// clear; the override slot is interior-mutable so this can be
    /// flipped per-@page without taking a `&mut Device`.
    pub fn set_paged_media_overrides(&self, overrides: Option<PagedMediaEnvOverrides>) {
        *self.paged_media_overrides.write() = overrides;
    }

    /// Read the currently installed paged-media env-var overrides.
    pub fn paged_media_overrides(&self) -> Option<PagedMediaEnvOverrides> {
        *self.paged_media_overrides.read()
    }
}

/// Controls how `env()` references are handled during variable substitution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentResolutionMode {
    /// Resolve `env()` references against the live device environment.
    ResolveLiveEnvironment,
    /// Treat `env()` references as unavailable so authored fallbacks, if any,
    /// are used instead.
    TreatAsMissing,
    /// Resolve only the CSS env-1 §3 paged-media variables
    /// (`page-width`, `page-height`, `page-margin-*`) against the
    /// device's installed [`PagedMediaEnvOverrides`]. Every other
    /// `env()` name is treated as unavailable so authored fallbacks
    /// (e.g. `env(safe-area-inset-top, 36pt)`) still win.
    ///
    /// Used by moegoe's `@page` rule re-cascade to substitute the
    /// per-@page used size and margins into authored `env(page-*)`
    /// calls without disturbing the safe-area-inset fallback path.
    ResolvePagedMediaEnvOnly,
}

type EnvironmentEvaluator = fn(device: &Device, url_data: &UrlExtraData) -> VariableValue;

struct EnvironmentVariable {
    name: Atom,
    evaluator: EnvironmentEvaluator,
}

macro_rules! make_variable {
    ($name:expr, $evaluator:expr) => {{
        EnvironmentVariable {
            name: $name,
            evaluator: $evaluator,
        }
    }};
}

fn get_safearea_inset_top(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(device.safe_area_insets().top, url_data)
}

fn get_safearea_inset_bottom(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(device.safe_area_insets().bottom, url_data)
}

fn get_safearea_inset_left(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(device.safe_area_insets().left, url_data)
}

fn get_safearea_inset_right(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(device.safe_area_insets().right, url_data)
}

// CSS env() variables for paged media (CSS env-1 §3.1).
//
// These resolve to the dimensions of the current @page area on
// per-page evaluation. Embedders that drive paged-media cascade
// (moegoe's paginator) install per-@page used geometry through
// [`CssEnvironment::set_paged_media_overrides`] before re-cascading
// the @page rule. When no override is installed the default `0px`
// flows through so `env(page-width, fallback)` parses and authored
// fallbacks take effect on the first pass.
fn paged_media_override_or_zero(
    device: &Device,
    select: fn(&PagedMediaEnvOverrides) -> Option<f32>,
    url_data: &UrlExtraData,
) -> VariableValue {
    let value = device
        .environment()
        .paged_media_overrides()
        .and_then(|overrides| select(&overrides))
        .unwrap_or(0.0);
    VariableValue::pixels(value, url_data)
}

fn get_page_width(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_width, url_data)
}

fn get_page_height(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_height, url_data)
}

fn get_page_margin_top(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_margin_top, url_data)
}

fn get_page_margin_right(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_margin_right, url_data)
}

fn get_page_margin_bottom(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_margin_bottom, url_data)
}

fn get_page_margin_left(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    paged_media_override_or_zero(device, |o| o.page_margin_left, url_data)
}

/// CSS env-1 §3 paged-media variable atoms. `CssEnvironment::get` uses
/// this set to filter what [`EnvironmentResolutionMode::ResolvePagedMediaEnvOnly`]
/// admits: only these names resolve under that mode; everything else
/// (notably `safe-area-inset-*`) falls back to authored values.
fn is_paged_media_env_name(name: &Atom) -> bool {
    *name == atom!("page-width")
        || *name == atom!("page-height")
        || *name == atom!("page-margin-top")
        || *name == atom!("page-margin-right")
        || *name == atom!("page-margin-bottom")
        || *name == atom!("page-margin-left")
}

#[cfg(feature = "gecko")]
fn get_content_preferred_color_scheme(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    use crate::queries::values::PrefersColorScheme;
    let prefers_color_scheme = unsafe {
        crate::gecko_bindings::bindings::Gecko_MediaFeatures_PrefersColorScheme(
            device.document(),
            /* use_content = */ true,
        )
    };
    VariableValue::ident(
        match prefers_color_scheme {
            PrefersColorScheme::Light => "light",
            PrefersColorScheme::Dark => "dark",
        },
        url_data,
    )
}

#[cfg(feature = "servo")]
fn get_content_preferred_color_scheme(_device: &Device, url_data: &UrlExtraData) -> VariableValue {
    // TODO: Add an implementation for Servo.
    VariableValue::ident("light", url_data)
}

fn get_scrollbar_inline_size(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(device.scrollbar_inline_size().px(), url_data)
}

fn get_hairline(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    VariableValue::pixels(
        app_units::Au(device.app_units_per_device_pixel()).to_f32_px(),
        url_data,
    )
}

static ENVIRONMENT_VARIABLES: [EnvironmentVariable; 10] = [
    make_variable!(atom!("safe-area-inset-top"), get_safearea_inset_top),
    make_variable!(atom!("safe-area-inset-bottom"), get_safearea_inset_bottom),
    make_variable!(atom!("safe-area-inset-left"), get_safearea_inset_left),
    make_variable!(atom!("safe-area-inset-right"), get_safearea_inset_right),
    // CSS env-1 §3 paged-media variables (default to 0px until
    // moegoe's paginator substitutes the live per-@page values).
    make_variable!(atom!("page-width"), get_page_width),
    make_variable!(atom!("page-height"), get_page_height),
    make_variable!(atom!("page-margin-top"), get_page_margin_top),
    make_variable!(atom!("page-margin-right"), get_page_margin_right),
    make_variable!(atom!("page-margin-bottom"), get_page_margin_bottom),
    make_variable!(atom!("page-margin-left"), get_page_margin_left),
];

#[cfg(feature = "gecko")]
macro_rules! lnf_int {
    ($id:ident) => {
        unsafe {
            crate::gecko_bindings::bindings::Gecko_GetLookAndFeelInt(
                crate::gecko_bindings::bindings::LookAndFeel_IntID::$id as i32,
            )
        }
    };
}

#[cfg(feature = "servo")]
macro_rules! lnf_int {
    ($id:ident) => {
        // TODO: Add an implementation for Servo.
        0
    };
}

macro_rules! lnf_int_variable {
    ($atom:expr, $id:ident, $ctor:ident) => {{
        fn __eval(_: &Device, url_data: &UrlExtraData) -> VariableValue {
            VariableValue::$ctor(lnf_int!($id), url_data)
        }
        make_variable!($atom, __eval)
    }};
}

fn eval_gtk_csd_titlebar_radius(device: &Device, url_data: &UrlExtraData) -> VariableValue {
    let int_pixels = lnf_int!(TitlebarRadius);
    let unzoomed_scale =
        device.device_pixel_ratio_ignoring_full_zoom().get() / device.device_pixel_ratio().get();
    VariableValue::pixels(int_pixels as f32 * unzoomed_scale, url_data)
}

static CHROME_ENVIRONMENT_VARIABLES: [EnvironmentVariable; 9] = [
    make_variable!(
        atom!("-moz-gtk-csd-titlebar-radius"),
        eval_gtk_csd_titlebar_radius
    ),
    lnf_int_variable!(
        atom!("-moz-gtk-csd-tooltip-radius"),
        TooltipRadius,
        int_pixels
    ),
    lnf_int_variable!(
        atom!("-moz-gtk-csd-close-button-position"),
        GTKCSDCloseButtonPosition,
        integer
    ),
    lnf_int_variable!(
        atom!("-moz-gtk-csd-minimize-button-position"),
        GTKCSDMinimizeButtonPosition,
        integer
    ),
    lnf_int_variable!(
        atom!("-moz-gtk-csd-maximize-button-position"),
        GTKCSDMaximizeButtonPosition,
        integer
    ),
    lnf_int_variable!(
        atom!("-moz-overlay-scrollbar-fade-duration"),
        ScrollbarFadeDuration,
        int_ms
    ),
    make_variable!(
        atom!("-moz-content-preferred-color-scheme"),
        get_content_preferred_color_scheme
    ),
    make_variable!(atom!("scrollbar-inline-size"), get_scrollbar_inline_size),
    make_variable!(atom!("hairline"), get_hairline),
];

impl CssEnvironment {
    #[inline]
    fn get(&self, name: &Atom, device: &Device, url_data: &UrlExtraData) -> Option<VariableValue> {
        if let Some(var) = ENVIRONMENT_VARIABLES.iter().find(|var| var.name == *name) {
            return Some((var.evaluator)(device, url_data));
        }
        if !url_data.chrome_rules_enabled() {
            return None;
        }
        let var = CHROME_ENVIRONMENT_VARIABLES
            .iter()
            .find(|var| var.name == *name)?;
        Some((var.evaluator)(device, url_data))
    }
}

/// A custom property name is just an `Atom`.
///
/// Note that this does not include the `--` prefix
pub type Name = Atom;

/// Parse a custom property name.
///
/// <https://drafts.csswg.org/css-variables/#typedef-custom-property-name>
pub fn parse_name(s: &str) -> Result<&str, ()> {
    if s.starts_with("--") && s.len() > 2 {
        Ok(&s[2..])
    } else {
        Err(())
    }
}

/// A value for a custom property is just a set of tokens.
///
/// We preserve the original CSS for serialization, and also the variable
/// references to other custom property names.
#[derive(Clone, Debug, MallocSizeOf, ToShmem)]
pub struct VariableValue {
    /// The raw CSS string.
    pub css: String,

    /// The url data of the stylesheet where this value came from.
    pub url_data: UrlExtraData,

    first_token_type: TokenSerializationType,
    last_token_type: TokenSerializationType,

    /// var(), env(), attr() or non-custom property (e.g. through `em`) references.
    references: References,

    /// Lexical names used when attr() or if() parses additional tokens at computed-value time.
    private_names: ThinVec<(Name, Name)>,

    /// The ranges of `css` that `attr()` substitution produced.
    attr_tainted: ThinVec<AttrTaintedRange>,
}

/// A byte range of a value that `attr()` substitution produced.
///
/// <https://drafts.csswg.org/css-values-5/#attr-security>
#[derive(Clone, Copy, Debug, Eq, MallocSizeOf, PartialEq, ToShmem)]
pub struct AttrTaintedRange {
    /// The first byte.
    pub start: usize,
    /// The byte after the last.
    pub end: usize,
}

trivial_to_computed_value!(VariableValue);

/// Given a potentially registered variable value turn it into a computed custom property value.
pub fn compute_variable_value(
    value: &Arc<VariableValue>,
    registration: &PropertyRegistrationData,
    computed_context: &computed::Context,
    name: &Name,
) -> Option<ComputedRegisteredValue> {
    if registration.syntax.is_universal() {
        return Some(ComputedRegisteredValue::universal(Arc::clone(value)));
    }
    compute_value(&value.css, &value.url_data, registration, computed_context, &format!("--{name}"))
        .ok()
        .map(|computed| computed.with_attr_taint(value.is_attr_tainted()))
}

// Identical authored tokens can refer to different unobservable mixin bindings.
impl PartialEq for VariableValue {
    fn eq(&self, other: &Self) -> bool {
        self.css == other.css
            && self.references == other.references
            && self.private_names == other.private_names
            && self.attr_tainted == other.attr_tainted
    }
}

impl Eq for VariableValue {}

impl ToCss for SpecifiedValue {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(&self.css)
    }
}

/// A pair of separate CustomPropertiesMaps, split between custom properties
/// that have the inherit flag set and those with the flag unset.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedCustomProperties {
    /// Map for custom properties with inherit flag set, including non-registered
    /// ones.
    pub inherited: CustomPropertiesMap,
    /// Map for custom properties with inherit flag unset.
    pub non_inherited: CustomPropertiesMap,
}

impl ComputedCustomProperties {
    /// Return whether the inherited and non_inherited maps are none.
    pub fn is_empty(&self) -> bool {
        self.inherited.is_empty() && self.non_inherited.is_empty()
    }

    /// Return the name and value of the property at specified index, if any.
    pub fn property_at(&self, index: usize) -> Option<(&Name, &Option<ComputedRegisteredValue>)> {
        // Just expose the custom property items from custom_properties.inherited, followed
        // by custom property items from custom_properties.non_inherited.
        self.inherited
            .get_index(index)
            .or_else(|| self.non_inherited.get_index(index - self.inherited.len()))
    }

    /// Insert a custom property in the corresponding inherited/non_inherited
    /// map, depending on whether the inherit flag is set or unset.
    pub(crate) fn insert(
        &mut self,
        registration: &PropertyRegistrationData,
        name: &Name,
        value: ComputedRegisteredValue,
    ) {
        self.map_mut(registration).insert(name, value)
    }

    /// Remove a custom property from the corresponding inherited/non_inherited
    /// map, depending on whether the inherit flag is set or unset.
    pub(crate) fn remove(&mut self, registration: &PropertyRegistrationData, name: &Name) {
        self.map_mut(registration).remove(name);
    }

    /// Shrink the capacity of the inherited maps as much as possible.
    fn shrink_to_fit(&mut self) {
        self.inherited.shrink_to_fit();
        self.non_inherited.shrink_to_fit();
    }

    fn map_mut(&mut self, registration: &PropertyRegistrationData) -> &mut CustomPropertiesMap {
        if registration.inherits() {
            &mut self.inherited
        } else {
            &mut self.non_inherited
        }
    }

    /// Returns the relevant custom property value given a registration.
    pub fn get(
        &self,
        registration: &PropertyRegistrationData,
        name: &Name,
    ) -> Option<&ComputedRegisteredValue> {
        if registration.inherits() {
            self.inherited.get(name)
        } else {
            self.non_inherited.get(name)
        }
    }
}

/// Both specified and computed values are VariableValues, the difference is
/// whether var() functions are expanded.
pub type SpecifiedValue = VariableValue;
/// Both specified and computed values are VariableValues, the difference is
/// whether var() functions are expanded.
pub type ComputedValue = VariableValue;

/// Set of flags to non-custom references this custom property makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, MallocSizeOf, ToShmem)]
struct NonCustomReferences(u8);

bitflags! {
    impl NonCustomReferences: u8 {
        /// At least one custom property depends on font-relative units.
        const FONT_UNITS = 1 << 0;
        /// At least one custom property depends on root element's font-relative units.
        const ROOT_FONT_UNITS = 1 << 1;
        /// At least one custom property depends on line height units.
        const LH_UNITS = 1 << 2;
        /// At least one custom property depends on root element's line height units.
        const ROOT_LH_UNITS = 1 << 3;
        /// The value contains currentcolor.
        const COLOR = 1 << 4;
        /// The value contains an accent system colour.
        const ACCENT_COLOR = 1 << 5;
        /// All dependencies not depending on the root element.
        const NON_ROOT_DEPENDENCIES = Self::FONT_UNITS.0 | Self::LH_UNITS.0 | Self::COLOR.0 | Self::ACCENT_COLOR.0;
        /// All dependencies depending on the root element.
        const ROOT_DEPENDENCIES = Self::ROOT_FONT_UNITS.0 | Self::ROOT_LH_UNITS.0;
    }
}

impl NonCustomReferences {
    /// The references that apply to an element: root-relative units only on the root.
    fn for_element(self, is_root_element: bool) -> Self {
        let mut mask = Self::NON_ROOT_DEPENDENCIES;
        if is_root_element {
            mask |= Self::ROOT_DEPENDENCIES
        }
        self & mask
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(SingleNonCustomReference),
    {
        for (_, r) in self.iter_names() {
            let single = match r {
                Self::FONT_UNITS => SingleNonCustomReference::FontUnits,
                Self::ROOT_FONT_UNITS => SingleNonCustomReference::RootFontUnits,
                Self::LH_UNITS => SingleNonCustomReference::LhUnits,
                Self::ROOT_LH_UNITS => SingleNonCustomReference::RootLhUnits,
                Self::COLOR => SingleNonCustomReference::Color,
                Self::ACCENT_COLOR => SingleNonCustomReference::AccentColor,
                _ => unreachable!("Unexpected single bit value"),
            };
            f(single);
        }
    }

    fn from_unit(value: &CowRcStr) -> Self {
        // For registered properties, any reference to font-relative dimensions
        // make it dependent on font-related properties.
        // TODO(dshin): When we unit algebra gets implemented and handled -
        // Is it valid to say that `calc(1em / 2em * 3px)` triggers this?
        if value.eq_ignore_ascii_case(FontRelativeLength::LH) {
            return Self::FONT_UNITS | Self::LH_UNITS;
        }
        if value.eq_ignore_ascii_case(FontRelativeLength::EM)
            || value.eq_ignore_ascii_case(FontRelativeLength::EX)
            || value.eq_ignore_ascii_case(FontRelativeLength::CAP)
            || value.eq_ignore_ascii_case(FontRelativeLength::CH)
            || value.eq_ignore_ascii_case(FontRelativeLength::IC)
        {
            return Self::FONT_UNITS;
        }
        if value.eq_ignore_ascii_case(FontRelativeLength::RLH) {
            return Self::ROOT_FONT_UNITS | Self::ROOT_LH_UNITS;
        }
        if value.eq_ignore_ascii_case(FontRelativeLength::REM)
            || value.eq_ignore_ascii_case(FontRelativeLength::REX)
            || value.eq_ignore_ascii_case(FontRelativeLength::RCH)
            || value.eq_ignore_ascii_case(FontRelativeLength::RCAP)
            || value.eq_ignore_ascii_case(FontRelativeLength::RIC)
        {
            return Self::ROOT_FONT_UNITS;
        }
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SingleNonCustomReference {
    FontUnits = 0,
    RootFontUnits,
    LhUnits,
    RootLhUnits,
    Color,
    AccentColor,
}

struct NonCustomReferenceMap<T>([Option<T>; 6]);

impl<T> Default for NonCustomReferenceMap<T> {
    fn default() -> Self {
        NonCustomReferenceMap(Default::default())
    }
}

impl<T> Index<SingleNonCustomReference> for NonCustomReferenceMap<T> {
    type Output = Option<T>;

    fn index(&self, reference: SingleNonCustomReference) -> &Self::Output {
        &self.0[reference as usize]
    }
}

impl<T> IndexMut<SingleNonCustomReference> for NonCustomReferenceMap<T> {
    fn index_mut(&mut self, reference: SingleNonCustomReference) -> &mut Self::Output {
        &mut self.0[reference as usize]
    }
}

/// Whether to defer resolving custom properties referencing font relative units.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum DeferFontRelativeCustomPropertyResolution {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToShmem, Parse)]
enum SubstitutionFunctionKind {
    Var,
    Env,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem, Parse)]
enum AttributeType {
    None,
    RawString,
    Type(Descriptor),
    Unit(AttrUnit),
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
enum SubstitutionFunction {
    Var(Name),
    DynamicVar {
        name: Option<Name>,
        arguments: Box<[FunctionArgument]>,
    },
    Ident(Box<[FunctionArgument]>),
    RandomItem {
        arguments: Box<[FunctionArgument]>,
        function_token_end: usize,
        identity: Option<crate::values::random::RandomFunctionIdentity>,
        selected_option: Option<usize>,
    },
    Inherit(Box<[FunctionArgument]>),
    If(Box<[ConditionalBranch]>),
    Env(Name),
    Attr {
        arguments: Box<[FunctionArgument]>,
        namespaces: Box<[(crate::Prefix, crate::Namespace)]>,
    },
    /// <https://drafts.csswg.org/css-mixins-1/#typedef-dashed-function>
    Function {
        name: DashedIdent,
        arguments: Box<[FunctionArgument]>,
        call_identity: Option<crate::values::random::RandomFunctionIdentity>,
    },
}

/// One `<dashed-function>` argument, without its `{}` wrapper.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
struct FunctionArgument {
    start: usize,
    end: usize,
    first_token_type: TokenSerializationType,
    last_token_type: TokenSerializationType,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
struct ConditionalBranch {
    condition: FunctionArgument,
    value: FunctionArgument,
    selected: Option<bool>,
    evaluated: bool,
}

impl SubstitutionFunction {
    fn variable_name(&self) -> Option<&Name> {
        match self {
            Self::Var(name) => Some(name),
            Self::DynamicVar { name, .. } => name.as_ref(),
            Self::Inherit(_)
            | Self::Ident(_)
            | Self::RandomItem { .. }
            | Self::If(_)
            | Self::Env(_)
            | Self::Attr { .. }
            | Self::Function { .. } => None,
        }
    }

    fn is_attr(&self) -> bool {
        matches!(self, Self::Attr { .. })
    }

    fn variable_name_mut(&mut self) -> Option<&mut Name> {
        match self {
            Self::Var(name) => Some(name),
            Self::DynamicVar { name, .. } => name.as_mut(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
struct VariableFallback {
    // NOTE(emilio): We don't track fallback end, because we rely on the missing closing
    // parenthesis, if any, to be inserted, which means that we can rely on our end being
    // reference.end - 1.
    start: num::NonZeroUsize,
    first_token_type: TokenSerializationType,
    last_token_type: TokenSerializationType,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
struct SubstitutionFunctionReference {
    function: SubstitutionFunction,
    start: usize,
    end: usize,
    fallback: Option<VariableFallback>,
    prev_token_type: TokenSerializationType,
    next_token_type: TokenSerializationType,
}

/// A struct holding information about the external references to that a custom
/// property value may have.
#[derive(Clone, Debug, Default, MallocSizeOf, PartialEq, ToShmem)]
struct References {
    refs: Vec<SubstitutionFunctionReference>,
    non_custom_references: NonCustomReferences,
    unit_references: Vec<(usize, NonCustomReferences)>,
    random_property: Option<Atom>,
    random_source_offset: usize,
    any_env: bool,
    any_var: bool,
    any_attr: bool,
    any_function: bool,
}

impl References {
    fn has_references(&self) -> bool {
        !self.refs.is_empty()
    }

    fn eager_non_custom_references(&self) -> NonCustomReferences {
        self.unit_references.iter().filter(|(position, _)| {
            !self.refs.iter().any(|outer| match &outer.function {
                SubstitutionFunction::RandomItem { arguments, selected_option, .. } => arguments.iter().skip(1).enumerate().any(|(index, argument)| {
                    *selected_option != Some(index) && *position >= argument.start && *position < argument.end
                }),
                SubstitutionFunction::If(branches) => branches.iter().any(|branch| branch.selected == Some(false) && *position >= branch.value.start && *position < branch.value.end),
                _ => false,
            })
        }).fold(NonCustomReferences::empty(), |all, (_, references)| all | *references)
    }

    fn in_lazy_argument(&self, reference: &SubstitutionFunctionReference) -> bool {
        self.refs.iter().any(|outer| {
            if let SubstitutionFunction::If(branches) = &outer.function {
                return branches.iter().any(|branch| branch.selected == Some(false) && reference.start >= branch.value.start && reference.end <= branch.value.end);
            }
            let (arguments, selected) = match &outer.function {
                SubstitutionFunction::Attr { arguments, .. } => (arguments, None),
                SubstitutionFunction::RandomItem { arguments, selected_option, .. } => (arguments, *selected_option),
                _ => return false,
            };
            arguments.iter().skip(1).enumerate().any(|(index, argument)| {
                selected != Some(index) && reference.start >= argument.start && reference.end <= argument.end
            })
        })
    }

}

/// Whether this name belongs to an unobservable lexical private property.
/// CSS tokenization replaces NUL, so authored identifiers cannot create these names.
pub fn is_private_name(name: &Name) -> bool {
    name.as_ref().starts_with('\0')
}

impl VariableValue {
    /// Statically named custom properties referenced by this value, including fallbacks.
    pub fn referenced_custom_properties(&self) -> impl Iterator<Item = &Name> {
        self.references.refs.iter().filter_map(|reference| reference.function.variable_name())
    }

    /// Rename lexical references without changing the retained authored token stream.
    pub fn rename_private_references(&mut self, names: &std::collections::HashMap<Name, Name>) {
        self.private_names = names
            .iter()
            .map(|(name, private)| (name.clone(), private.clone()))
            .collect();
        for reference in &mut self.references.refs {
            if let Some(name) = reference.function.variable_name_mut() {
                if let Some(private) = names.get(name) {
                    *name = private.clone();
                }
            }
        }
    }

    /// Construct an unobservable function call from already validated argument values.
    pub fn private_function_call(
        name: Atom,
        arguments: &[Arc<VariableValue>],
        url_data: &UrlExtraData,
    ) -> Option<Self> {
        let css = format!(
            "--internal({})",
            arguments
                .iter()
                .map(|value| format!("{{{}}}", value.css))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut input = cssparser::ParserInput::new(&css);
        let mut input = cssparser::Parser::new(&mut input);
        let mut value = Self::parse(&mut input, url_data, &Namespaces::default()).ok()?;
        if let SubstitutionFunction::Function {
            name: ref mut function,
            ..
        } = value.references.refs.first_mut()?.function
        {
            function.0 = name;
        }
        for (target, source) in value.references.refs.iter_mut().skip(1).zip(
            arguments
                .iter()
                .flat_map(|argument| argument.references.refs.iter()),
        ) {
            match (&mut target.function, &source.function) {
                (SubstitutionFunction::Var(target), SubstitutionFunction::Var(source)) => {
                    *target = source.clone()
                },
                (
                    SubstitutionFunction::DynamicVar { name: target, .. },
                    SubstitutionFunction::Var(source),
                ) => *target = Some(source.clone()),
                (
                    SubstitutionFunction::Var(target),
                    SubstitutionFunction::DynamicVar {
                        name: Some(source), ..
                    },
                ) => *target = source.clone(),
                (
                    SubstitutionFunction::DynamicVar { name: target, .. },
                    SubstitutionFunction::DynamicVar { name: source, .. },
                ) => *target = source.clone(),
                (
                    SubstitutionFunction::Function { name: target, call_identity: target_identity, .. },
                    SubstitutionFunction::Function { name: source, call_identity: source_identity, .. },
                ) => { *target = source.clone(); *target_identity = source_identity.clone(); },
                (
                    SubstitutionFunction::RandomItem { identity: target, .. },
                    SubstitutionFunction::RandomItem { identity: source, .. },
                ) => *target = source.clone(),
                _ => {},
            }
        }
        Some(value)
    }

    /// Construct a variable reference using an unforgeable name, retaining valid CSS offsets.
    pub fn private_variable_reference(name: Name, url_data: &UrlExtraData) -> Self {
        let mut input = cssparser::ParserInput::new("var(--internal)");
        let mut input = cssparser::Parser::new(&mut input);
        let mut value = Self::parse(&mut input, url_data, &Namespaces::default()).unwrap();
        value.references.refs[0].function = SubstitutionFunction::Var(name);
        value
    }

    /// The already parsed arguments of a single dashed function invocation.
    pub fn function_arguments(&self) -> Option<Vec<Arc<VariableValue>>> {
        let reference = self.references.refs.first()?;
        let SubstitutionFunction::Function { ref arguments, .. } = reference.function else {
            return None;
        };
        arguments
            .iter()
            .map(|argument| {
                let mut input =
                    cssparser::ParserInput::new(&self.css[argument.start..argument.end]);
                let mut input = cssparser::Parser::new(&mut input);
                let mut value = Self::parse(&mut input, &self.url_data, &Namespaces::default()).ok()?;
                value.copy_random_function_context(self.references.refs.iter(), argument.start);
                value.references.random_property = self.references.random_property.clone();
                Some(Arc::new(value))
            })
            .collect()
    }

    fn copy_random_function_context<'r>(&mut self, source: impl Iterator<Item = &'r SubstitutionFunctionReference> + Clone, offset: usize) {
        for target in &mut self.references.refs {
            let Some(original) = source.clone().find(|original| original.start == offset + target.start) else { continue; };
            match (&mut target.function, &original.function) {
                (SubstitutionFunction::RandomItem { identity: target, .. }, SubstitutionFunction::RandomItem { identity: original, .. })
                | (SubstitutionFunction::Function { call_identity: target, .. }, SubstitutionFunction::Function { call_identity: original, .. }) => *target = original.clone(),
                _ => {},
            }
        }
    }

    fn empty(url_data: &UrlExtraData) -> Self {
        Self {
            css: String::new(),
            last_token_type: Default::default(),
            first_token_type: Default::default(),
            url_data: url_data.clone(),
            references: Default::default(),
            attr_tainted: ThinVec::new(),
            private_names: ThinVec::new(),
        }
    }

    /// Create a new custom property without parsing if the CSS is known to be valid and contain no
    /// references.
    pub fn new(
        css: String,
        url_data: &UrlExtraData,
        first_token_type: TokenSerializationType,
        last_token_type: TokenSerializationType,
    ) -> Self {
        Self {
            css,
            url_data: url_data.clone(),
            first_token_type,
            last_token_type,
            references: Default::default(),
            attr_tainted: ThinVec::new(),
            private_names: ThinVec::new(),
        }
    }

    fn push<'i>(
        &mut self,
        css: &str,
        css_first_token_type: TokenSerializationType,
        css_last_token_type: TokenSerializationType,
        attr_tainted: &[AttrTaintedRange],
    ) -> Result<(), ()> {
        /// Prevent values from getting terribly big since you can use custom
        /// properties exponentially.
        ///
        /// This number (2MB) is somewhat arbitrary, but silly enough that no
        /// reasonable page should hit it. We could limit by number of total
        /// substitutions, but that was very easy to work around in practice
        /// (just choose a larger initial value and boom).
        const MAX_VALUE_LENGTH_IN_BYTES: usize = 2 * 1024 * 1024;

        if self.css.len() + css.len() > MAX_VALUE_LENGTH_IN_BYTES {
            return Err(());
        }

        // This happens e.g. between two subsequent var() functions:
        // `var(--a)var(--b)`.
        //
        // In that case, css_*_token_type is nonsensical.
        if css.is_empty() {
            return Ok(());
        }

        self.first_token_type.set_if_nothing(css_first_token_type);
        // If self.first_token_type was nothing,
        // self.last_token_type is also nothing and this will be false:
        if self
            .last_token_type
            .needs_separator_when_before(css_first_token_type)
        {
            self.css.push_str("/**/")
        }
        let offset = self.css.len();
        self.attr_tainted
            .extend(attr_tainted.iter().map(|range| AttrTaintedRange {
                start: offset + range.start,
                end: offset + range.end,
            }));
        self.css.push_str(css);
        self.last_token_type = css_last_token_type;
        Ok(())
    }

    /// Parse a custom property value.
    pub fn parse<'i, 't>(
        input: &mut Parser<'i, 't>,
        url_data: &UrlExtraData,
        namespaces: &Namespaces,
    ) -> Result<Self, ParseError<'i>> {
        let mut references = References::default();
        let mut missing_closing_characters = String::new();
        input.skip_whitespace();
        let start_position = input.position();
        let (first_token_type, last_token_type) = parse_declaration_value(
            input,
            start_position,
            namespaces,
            &mut references,
            &mut missing_closing_characters,
        )?;
        let mut css = input.slice_from(start_position).to_owned();
        let consumed_eof_escape =
            css.ends_with("\\") && missing_closing_characters.starts_with('�');
        if !missing_closing_characters.is_empty() {
            // EOF consumes an unescaped backslash: strings ignore it, while
            // identifiers, dimensions and URLs replace it with U+FFFD.
            if css.ends_with("\\")
                && (consumed_eof_escape
                    || matches!(missing_closing_characters.as_bytes()[0], b'"' | b'\''))
            {
                css.pop();
            }
            css.push_str(&missing_closing_characters);
        }
        if consumed_eof_escape {
            // Removing the consumed source byte also changes any enclosing
            // substitution ranges. Parse the repaired value once so all ranges
            // refer to its actual serialization, including generated closers.
            let mut repaired = ParserInput::new(&css);
            return Self::parse(&mut Parser::new(&mut repaired), url_data, namespaces)
                .map_err(|_| input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }

        css.truncate(trim_token_whitespace(&css).len());
        css.shrink_to_fit();
        references.refs.shrink_to_fit();

        Ok(Self {
            css,
            url_data: url_data.clone(),
            first_token_type,
            last_token_type,
            references,
            attr_tainted: ThinVec::new(),
            private_names: ThinVec::new(),
        })
    }

    /// Create VariableValue from an int.
    fn integer(number: i32, url_data: &UrlExtraData) -> Self {
        Self::from_token(
            Token::Number {
                has_sign: false,
                value: number as f32,
                int_value: Some(number),
            },
            url_data,
        )
    }

    /// Create VariableValue from an int.
    fn ident(ident: &'static str, url_data: &UrlExtraData) -> Self {
        Self::from_token(Token::Ident(ident.into()), url_data)
    }

    /// Create VariableValue from a float amount of CSS pixels.
    fn pixels(number: f32, url_data: &UrlExtraData) -> Self {
        // FIXME (https://github.com/servo/rust-cssparser/issues/266):
        // No way to get TokenSerializationType::Dimension without creating
        // Token object.
        Self::from_token(
            Token::Dimension {
                has_sign: false,
                value: number,
                int_value: None,
                unit: CowRcStr::from("px"),
            },
            url_data,
        )
    }

    /// Create VariableValue from an integer amount of milliseconds.
    fn int_ms(number: i32, url_data: &UrlExtraData) -> Self {
        Self::from_token(
            Token::Dimension {
                has_sign: false,
                value: number as f32,
                int_value: Some(number),
                unit: CowRcStr::from("ms"),
            },
            url_data,
        )
    }

    /// Create VariableValue from an integer amount of CSS pixels.
    fn int_pixels(number: i32, url_data: &UrlExtraData) -> Self {
        Self::from_token(
            Token::Dimension {
                has_sign: false,
                value: number as f32,
                int_value: Some(number),
                unit: CowRcStr::from("px"),
            },
            url_data,
        )
    }

    fn from_token(token: Token, url_data: &UrlExtraData) -> Self {
        let token_type = token.serialization_type();
        let mut css = token.to_css_string();
        css.shrink_to_fit();

        VariableValue {
            css,
            url_data: url_data.clone(),
            first_token_type: token_type,
            last_token_type: token_type,
            references: Default::default(),
            attr_tainted: ThinVec::new(),
            private_names: ThinVec::new(),
        }
    }

    /// Retain the declaration identity before substitution changes token offsets.
    pub fn set_random_function_context(&mut self, context: &crate::parser::ParserContext, declaration_value_start: SourcePosition) {
        let property = context.random_declaration_property().cloned();
        self.references.random_property = property.clone();
        let mut calls = 0;
        for reference in &mut self.references.refs {
            match &mut reference.function {
                SubstitutionFunction::RandomItem { identity, function_token_end, .. } => {
                    *identity = Some(context.random_early_function_identity_by_byte(declaration_value_start.byte_index() + *function_token_end));
                },
                SubstitutionFunction::Function { call_identity, .. } => {
                    calls += 1;
                    *call_identity = property.as_ref().map(|property| crate::values::random::RandomFunctionIdentity { property: property.clone(), index: calls });
                },
                _ => {},
            }
        }
    }

    pub(super) fn scope_random_functions(&mut self, function: &str) {
        if let Some(property) = &mut self.references.random_property {
            *property = Atom::from(format!("{function}-{property}"));
        }
        for reference in &mut self.references.refs {
            let identity = match &mut reference.function {
                SubstitutionFunction::RandomItem { identity, .. } => identity,
                SubstitutionFunction::Function { call_identity, .. } => call_identity,
                _ => continue,
            };
            if let Some(identity) = identity {
                identity.property = Atom::from(if let Some(property) = identity.property.strip_prefix("early-") {
                    format!("early-{function}-{property}")
                } else { format!("{function}-{}", identity.property) });
            }
        }
    }

    /// Returns the raw CSS text from this VariableValue
    pub fn css_text(&self) -> &str {
        &self.css
    }

    /// Returns whether this variable value has any reference to the environment or other
    /// variables.
    pub fn has_references(&self) -> bool {
        self.references.has_references()
    }

    pub(crate) fn is_attr_tainted(&self) -> bool {
        !self.attr_tainted.is_empty()
    }

    pub(crate) fn with_attr_taint(mut self, tainted: bool) -> Self {
        if tainted {
            self.attr_tainted = ThinVec::from([AttrTaintedRange {
                start: 0,
                end: self.css.len(),
            }]);
        }
        self
    }

    /// Returns whether this value calls a custom function.
    pub fn has_dashed_functions(&self) -> bool {
        self.references.any_function
    }
}

/// <https://drafts.csswg.org/css-syntax-3/#typedef-declaration-value>
fn parse_declaration_value<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<(TokenSerializationType, TokenSerializationType), ParseError<'i>> {
    input.parse_until_before(Delimiter::Bang | Delimiter::Semicolon, |input| {
        parse_declaration_value_block(
            input,
            input_start,
            namespaces,
            references,
            missing_closing_characters,
        )
    })
}

/// Like parse_declaration_value, but accept `!` and `;` since they are only invalid at the top level.
fn parse_declaration_value_block<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<(TokenSerializationType, TokenSerializationType), ParseError<'i>> {
    let mut is_first = true;
    let mut first_token_type = TokenSerializationType::Nothing;
    let mut last_token_type = TokenSerializationType::Nothing;
    let mut prev_reference_index: Option<usize> = None;
    loop {
        let token_start = input.position();
        let Ok(token) = input.next_including_whitespace_and_comments() else {
            break;
        };

        let prev_token_type = last_token_type;
        let serialization_type = token.serialization_type();
        last_token_type = serialization_type;
        if is_first {
            first_token_type = last_token_type;
            is_first = false;
        }

        macro_rules! nested {
            ($closing:expr) => {{
                let mut inner_end_position = None;
                let result = input.parse_nested_block(|input| {
                    let result = parse_declaration_value_block(
                        input,
                        input_start,
                        namespaces,
                        references,
                        missing_closing_characters,
                    )?;
                    inner_end_position = Some(input.position());
                    Ok(result)
                })?;
                if inner_end_position.unwrap() == input.position() {
                    missing_closing_characters.push_str($closing);
                }
                result
            }};
        }
        if let Some(index) = prev_reference_index.take() {
            references.refs[index].next_token_type = serialization_type;
        }
        match *token {
            Token::Comment(_) => {
                let token_slice = input.slice_from(token_start);
                if !token_slice.ends_with("*/") {
                    missing_closing_characters.push_str(if token_slice.ends_with('*') {
                        "/"
                    } else {
                        "*/"
                    })
                }
            },
            Token::BadUrl(ref u) => {
                let e = StyleParseErrorKind::BadUrlInDeclarationValueBlock(u.clone());
                return Err(input.new_custom_error(e));
            },
            Token::BadString(ref s) => {
                let e = StyleParseErrorKind::BadStringInDeclarationValueBlock(s.clone());
                return Err(input.new_custom_error(e));
            },
            Token::CloseParenthesis => {
                let e = StyleParseErrorKind::UnbalancedCloseParenthesisInDeclarationValueBlock;
                return Err(input.new_custom_error(e));
            },
            Token::CloseSquareBracket => {
                let e = StyleParseErrorKind::UnbalancedCloseSquareBracketInDeclarationValueBlock;
                return Err(input.new_custom_error(e));
            },
            Token::CloseCurlyBracket => {
                let e = StyleParseErrorKind::UnbalancedCloseCurlyBracketInDeclarationValueBlock;
                return Err(input.new_custom_error(e));
            },
            Token::Function(ref name)
                if name.starts_with("--")
                    || name.eq_ignore_ascii_case("inherit")
                    || name.eq_ignore_ascii_case("if")
                    || name.eq_ignore_ascii_case("var")
                    || name.eq_ignore_ascii_case("ident")
                    || name.eq_ignore_ascii_case("random-item")
                    || (name.eq_ignore_ascii_case("attr")
                        && static_prefs::pref!("layout.css.attr.enabled")) =>
            {
                let name = name.clone();
                let is_if = name.eq_ignore_ascii_case("if");
                let is_inherit = name.eq_ignore_ascii_case("inherit");
                let is_attr = name.eq_ignore_ascii_case("attr");
                let is_var = name.eq_ignore_ascii_case("var");
                let is_ident = name.eq_ignore_ascii_case("ident");
                let is_random_item = name.eq_ignore_ascii_case("random-item");
                let function_token_end = input.position().byte_index() - input_start.byte_index();
                let our_ref_index = references.refs.len();
                let start = token_start.byte_index() - input_start.byte_index();
                references.refs.push(SubstitutionFunctionReference {
                    function: if name.starts_with("--") {
                        SubstitutionFunction::Function {
                            name: DashedIdent(Atom::from(name.as_ref())),
                            arguments: Box::default(),
                            call_identity: None,
                        }
                    } else if name.eq_ignore_ascii_case("inherit") {
                        SubstitutionFunction::Inherit(Box::default())
                    } else if is_attr {
                        SubstitutionFunction::Attr {
                            arguments: Box::default(),
                            namespaces: namespaces
                                .prefixes
                                .iter()
                                .map(|(prefix, namespace)| (prefix.clone(), namespace.clone()))
                                .collect(),
                        }
                    } else if is_var {
                        SubstitutionFunction::DynamicVar {
                            name: None,
                            arguments: Box::default(),
                        }
                    } else if is_ident {
                        SubstitutionFunction::Ident(Box::default())
                    } else if is_random_item {
                        SubstitutionFunction::RandomItem {
                            arguments: Box::default(),
                            function_token_end,
                            identity: None,
                            selected_option: None,
                        }
                    } else {
                        SubstitutionFunction::If(Box::default())
                    },
                    start,
                    end: start,
                    prev_token_type,
                    next_token_type: TokenSerializationType::Nothing,
                    fallback: None,
                });
                let mut input_end_position = None;
                let parsed_arguments = input.parse_nested_block(|input| {
                    if is_if {
                        let branches = conditionals::parse_branches(
                            input,
                            input_start,
                            namespaces,
                            references,
                            missing_closing_characters,
                        )?;
                        references.refs[our_ref_index].function =
                            SubstitutionFunction::If(branches);
                        input_end_position = Some(input.position());
                        return Ok(Box::default());
                    }
                    if is_random_item {
                        let mut arguments = vec![input.parse_until_before(Delimiter::Comma, |input| {
                            parse_component_argument(input, input_start, namespaces, references, missing_closing_characters, false)
                        })?];
                        input.expect_comma()?;
                        loop {
                            arguments.push(input.parse_until_before(Delimiter::Comma, |input| {
                                parse_component_argument(input, input_start, namespaces, references, missing_closing_characters, true)
                            })?);
                            if input.is_exhausted() { break; }
                            input.expect_comma()?;
                        }
                        input_end_position = Some(input.position());
                        return Ok(arguments.into_boxed_slice());
                    }
                    if is_ident {
                        let state = input.state();
                        while let Ok(token) = input.next() {
                            if matches!(token, Token::CurlyBracketBlock | Token::Comma) {
                                return Err(
                                    input.new_custom_error(StyleParseErrorKind::UnspecifiedError)
                                );
                            }
                        }
                        input.reset(&state);
                        let argument = input.parse_until_before(Delimiter::Comma, |input| {
                            parse_function_argument_value(
                                input,
                                input_start,
                                namespaces,
                                references,
                                missing_closing_characters,
                            )
                        })?;
                        input.expect_exhausted()?;
                        input_end_position = Some(input.position());
                        return Ok(vec![argument].into_boxed_slice());
                    }
                    if is_inherit || is_attr || is_var {
                        let mut arguments =
                            vec![input.parse_until_before(Delimiter::Comma, |input| {
                                parse_component_argument(
                                    input,
                                    input_start,
                                    namespaces,
                                    references,
                                    missing_closing_characters,
                                    false,
                                )
                            })?];
                        if !input.is_exhausted() {
                            input.expect_comma()?;
                            arguments.push(parse_component_argument(
                                input,
                                input_start,
                                namespaces,
                                references,
                                missing_closing_characters,
                                true,
                            )?);
                        }
                        input_end_position = Some(input.position());
                        return Ok(arguments.into_boxed_slice());
                    }
                    let arguments = parse_function_arguments(
                        input,
                        input_start,
                        namespaces,
                        references,
                        missing_closing_characters,
                    )?;
                    input_end_position = Some(input.position());
                    Ok(arguments)
                })?;
                if input_end_position.unwrap() == input.position() {
                    missing_closing_characters.push_str(")");
                }
                let static_var_name = if is_var {
                    let argument = &parsed_arguments[0];
                    let name = parse_property_name_css(
                        &input.slice_from(input_start)[argument.start..argument.end],
                    );
                    name
                } else {
                    None
                };
                prev_reference_index = Some(our_ref_index);
                let reference = &mut references.refs[our_ref_index];
                reference.end = input.position().byte_index() - input_start.byte_index()
                    + missing_closing_characters.len();
                match reference.function {
                    SubstitutionFunction::Function {
                        ref mut arguments, ..
                    } => {
                        *arguments = parsed_arguments;
                        references.any_function = true;
                    },
                    SubstitutionFunction::Inherit(ref mut arguments) => {
                        *arguments = parsed_arguments;
                        references.any_var = true;
                    },
                    SubstitutionFunction::DynamicVar {
                        ref mut name,
                        ref mut arguments,
                    } => {
                        *name = static_var_name;
                        *arguments = parsed_arguments;
                        references.any_var = true;
                    },
                    SubstitutionFunction::Ident(ref mut arguments)
                    | SubstitutionFunction::RandomItem { ref mut arguments, .. } => {
                        *arguments = parsed_arguments;
                        references.any_var = true;
                    },
                    SubstitutionFunction::Attr {
                        ref mut arguments, ..
                    } => {
                        *arguments = parsed_arguments;
                        references.any_attr = true;
                    },
                    SubstitutionFunction::If(_) => references.any_var = true,
                    _ => unreachable!(),
                }
            },
            Token::Function(ref name) => {
                let substitution_kind = SubstitutionFunctionKind::from_ident(name).ok();
                if let Some(substitution_kind) = substitution_kind {
                    let our_ref_index = references.refs.len();
                    let mut input_end_position = None;
                    let fallback = input.parse_nested_block(|input| {
                        // TODO(emilio): For env() this should be <custom-ident> per spec, but no other browser does
                        // that, see https://github.com/w3c/csswg-drafts/issues/3262.
                        let function = match substitution_kind {
                            SubstitutionFunctionKind::Var => {
                                let name = input.expect_ident()?;
                                let name = match parse_name(name.as_ref()) {
                                    Ok(name) => name,
                                    Err(()) => {
                                        let name = name.clone();
                                        return Err(input.new_custom_error(
                                            SelectorParseErrorKind::UnexpectedIdent(name),
                                        ));
                                    },
                                };
                                SubstitutionFunction::Var(Atom::from(name))
                            },
                            SubstitutionFunctionKind::Env => SubstitutionFunction::Env(Atom::from(
                                input.expect_ident()?.as_ref(),
                            )),
                        };

                        // We want the order of the references to match source order. So we need to reserve our slot
                        // now, _before_ parsing our fallback. Note that we don't care if parsing fails after all, since
                        // if this fails we discard the whole result anyways.
                        let start = token_start.byte_index() - input_start.byte_index();
                        references.refs.push(SubstitutionFunctionReference {
                            function,
                            start,
                            // To be fixed up after parsing fallback and auto-closing via our_ref_index.
                            end: start,
                            prev_token_type,
                            // To be fixed up (if needed) on the next loop iteration via prev_reference_index.
                            next_token_type: TokenSerializationType::Nothing,
                            // To be fixed up after parsing fallback.
                            fallback: None,
                        });

                        let mut fallback = None;
                        if input.try_parse(|input| input.expect_comma()).is_ok() {
                            input.skip_whitespace();
                            let fallback_start = num::NonZeroUsize::new(
                                input.position().byte_index() - input_start.byte_index(),
                            )
                            .unwrap();
                            // NOTE(emilio): Intentionally using parse_declaration_value rather than
                            // parse_declaration_value_block, since that's what parse_fallback used to do.
                            let (first, last) = parse_declaration_value(
                                input,
                                input_start,
                                namespaces,
                                references,
                                missing_closing_characters,
                            )?;
                            fallback = Some(VariableFallback {
                                start: fallback_start,
                                first_token_type: first,
                                last_token_type: last,
                            });
                            input_end_position = Some(input.position());
                        } else {
                            let state = input.state();
                            // We still need to consume the rest of the potentially-unclosed
                            // tokens, but make sure to not consume tokens that would otherwise be
                            // invalid, by calling reset().
                            parse_declaration_value_block(
                                input,
                                input_start,
                                namespaces,
                                references,
                                missing_closing_characters,
                            )?;
                            input_end_position = Some(input.position());
                            input.reset(&state);
                        }
                        Ok(fallback)
                    })?;
                    if input_end_position.unwrap() == input.position() {
                        missing_closing_characters.push_str(")");
                    }
                    prev_reference_index = Some(our_ref_index);
                    let reference = &mut references.refs[our_ref_index];
                    reference.end = input.position().byte_index() - input_start.byte_index()
                        + missing_closing_characters.len();
                    reference.fallback = fallback;
                    match substitution_kind {
                        SubstitutionFunctionKind::Var => references.any_var = true,
                        SubstitutionFunctionKind::Env => references.any_env = true,
                    };
                } else {
                    nested!(")");
                }
            },
            Token::ParenthesisBlock => {
                nested!(")");
            },
            Token::CurlyBracketBlock => {
                nested!("}");
            },
            Token::SquareBracketBlock => {
                nested!("]");
            },
            Token::QuotedString(_) => {
                let token_slice = input.slice_from(token_start);
                let quote = &token_slice[..1];
                debug_assert!(matches!(quote, "\"" | "'"));
                if !(token_slice.ends_with(quote) && token_slice.len() > 1) {
                    missing_closing_characters.push_str(quote)
                }
            },
            Token::Ident(ref value)
            | Token::AtKeyword(ref value)
            | Token::Hash(ref value)
            | Token::IDHash(ref value)
            | Token::UnquotedUrl(ref value)
            | Token::Dimension {
                unit: ref value, ..
            } => {
                let mut unit = NonCustomReferences::from_unit(value);
                if matches!(token, Token::Ident(_)) {
                    if value.eq_ignore_ascii_case("currentcolor") {
                        unit.insert(NonCustomReferences::COLOR);
                    } else if value.eq_ignore_ascii_case("accentcolor")
                        || value.eq_ignore_ascii_case("accentcolortext")
                    {
                        unit.insert(NonCustomReferences::ACCENT_COLOR);
                    }
                }
                references.non_custom_references.insert(unit);
                if !unit.is_empty() {
                    references.unit_references.push((token_start.byte_index() - input_start.byte_index(), unit));
                }
                let is_unquoted_url = matches!(token, Token::UnquotedUrl(_));
                if value.ends_with("�") && input.slice_from(token_start).ends_with("\\") {
                    // Unescaped backslash at EOF in these contexts is interpreted as U+FFFD
                    // Check the value in case the final backslash was itself escaped.
                    // VariableValue::parse removes the consumed source backslash
                    // before appending the replacement and any closing delimiters.
                    missing_closing_characters.push_str("�")
                }
                if is_unquoted_url && !input.slice_from(token_start).ends_with(")") {
                    missing_closing_characters.push_str(")");
                }
            },
            _ => {},
        };
    }
    Ok((first_token_type, last_token_type))
}

/// Parse `<declaration-value>#?`, the arguments of a `<dashed-function>`.
/// <https://drafts.csswg.org/css-values-5/#component-function-commas>
fn parse_function_arguments<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<Box<[FunctionArgument]>, ParseError<'i>> {
    input.skip_whitespace();
    if input.is_exhausted() {
        return Ok(Box::default());
    }
    let mut arguments = vec![];
    loop {
        arguments.push(input.parse_until_before(Delimiter::Comma, |input| {
            parse_function_argument(
                input,
                input_start,
                namespaces,
                references,
                missing_closing_characters,
            )
        })?);
        if input.next().is_err() {
            return Ok(arguments.into_boxed_slice());
        }
    }
}

fn parse_function_argument<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<FunctionArgument, ParseError<'i>> {
    input.skip_whitespace();
    let state = input.state();
    let named = input.try_parse(|input| -> Result<bool, ParseError<'i>> {
        let dashed = input.expect_ident()?.starts_with("--");
        input.expect_colon()?;
        Ok(dashed)
    });
    if matches!(named, Ok(true)) {
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    input.reset(&state);
    parse_component_argument(input, input_start, namespaces, references, missing_closing_characters, false)
}

fn parse_function_argument_value<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<FunctionArgument, ParseError<'i>> {
    let argument = parse_substitution_argument(
        input,
        input_start,
        namespaces,
        references,
        missing_closing_characters,
    )?;
    if argument.start == argument.end {
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(argument)
}

fn parse_component_argument<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
    allow_empty: bool,
) -> Result<FunctionArgument, ParseError<'i>> {
    input.skip_whitespace();
    if input.try_parse(|input| input.expect_curly_bracket_block()).is_ok() {
        let mut inner_end_position = None;
        let argument = input.parse_nested_block(|input| {
            let argument = parse_substitution_argument(input, input_start, namespaces, references, missing_closing_characters)?;
            if !allow_empty && argument.start == argument.end {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            inner_end_position = Some(input.position());
            Ok(argument)
        })?;
        if inner_end_position == Some(input.position()) {
            missing_closing_characters.push_str("}");
        }
        input.expect_exhausted()?;
        return Ok(argument);
    }
    let state = input.state();
    while let Ok(token) = input.next() {
        if matches!(token, Token::CurlyBracketBlock) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
    }
    input.reset(&state);
    let argument = parse_substitution_argument(input, input_start, namespaces, references, missing_closing_characters)?;
    if !allow_empty && argument.start == argument.end {
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(argument)
}

fn parse_substitution_argument<'i, 't>(
    input: &mut Parser<'i, 't>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<FunctionArgument, ParseError<'i>> {
    input.skip_whitespace();
    let start = input.position();
    parse_declaration_value(
        input,
        input_start,
        namespaces,
        references,
        missing_closing_characters,
    )?;
    input.expect_exhausted()?;
    let css = input.slice_from(start);
    let (first_token_type, last_token_type) = token_types(css);
    let start = start.byte_index() - input_start.byte_index();
    Ok(FunctionArgument {
        start,
        end: start + css.len(),
        first_token_type,
        last_token_type,
    })
}

pub(crate) fn trim_token_whitespace(css: &str) -> &str {
    if !css.starts_with(crate::str::char_is_whitespace) && !css.ends_with(crate::str::char_is_whitespace) {
        return css;
    }
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    let mut first = None;
    let mut end = 0;
    loop {
        let start = input.position();
        let Ok(token) = input.next_including_whitespace_and_comments() else { break; };
        match token {
            Token::WhiteSpace(_) => continue,
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let _ = input.parse_nested_block(|input| {
                    while input.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<(), cssparser::ParseError<'_, ()>>(())
                });
            },
            _ => {},
        }
        first.get_or_insert(start.byte_index());
        end = input.position().byte_index();
    }
    &css[first.unwrap_or(end)..end]
}

fn token_types(css: &str) -> (TokenSerializationType, TokenSerializationType) {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    let mut first = TokenSerializationType::Nothing;
    let mut last = TokenSerializationType::Nothing;
    while let Ok(token) = input.next_including_whitespace_and_comments() {
        last = token.serialization_type();
        first.set_if_nothing(last);
    }
    (first, last)
}

fn parse_property_name_css(css: &str) -> Option<Name> {
    let mut input = ParserInput::new(css);
    Parser::new(&mut input)
        .parse_entirely(|input| {
            let name = input.expect_ident()?;
            parse_name(name)
                .map(Atom::from)
                .map_err(|()| input.new_error_for_next_token::<()>())
        })
        .ok()
}

/// Parse <attr-type> = type( <syntax> ) | raw-string | number | <attr-unit>.
/// https://drafts.csswg.org/css-values-5/#attr-notation
fn parse_attr_type<'i, 't>(input: &mut Parser<'i, 't>) -> Result<AttributeType, ParseError<'i>> {
    if input.is_exhausted() {
        return Ok(AttributeType::None);
    }
    Ok(match input.next()? {
        Token::Function(ref name) if name.eq_ignore_ascii_case("type") => {
            let syntax = input.parse_nested_block(Descriptor::from_css_parser)?;
            if syntax.components.iter().any(|component| {
                matches!(
                    component.name(),
                    crate::properties_and_values::syntax::ComponentName::DataType(
                        crate::properties_and_values::syntax::data_type::DataType::Url
                    )
                )
            }) {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            AttributeType::Type(syntax)
        },
        Token::Ident(ref ident) => {
            if ident.eq_ignore_ascii_case("raw-string") {
                AttributeType::RawString
            } else if ident.eq_ignore_ascii_case("-bd-ident") {
                AttributeType::Type(Descriptor::from_str("<custom-ident>", false)
                    .map_err(|_| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))?)
            } else {
                let unit = AttrUnit::from_ident(ident)
                    .map_err(|_| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))?;
                AttributeType::Unit(unit)
            }
        },
        Token::Delim('%') => AttributeType::Unit(AttrUnit::Percentage),
        _ => return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
    })
}

/// A struct that takes care of encapsulating the cascade process for custom properties.
pub struct CustomPropertiesBuilder<'a, 'b: 'a> {
    seen: PrecomputedHashSet<&'a Name>,
    may_have_cycles: bool,
    has_color_scheme: bool,
    custom_properties: ComputedCustomProperties,
    reverted: PrecomputedHashMap<&'a Name, (CascadePriority, bool)>,
    reverted_rules: PrecomputedHashMap<&'a Name, std::collections::HashMap<usize, CascadePriority>>,
    stylist: &'a Stylist,
    computed_context: &'a mut computed::Context<'b>,
    references_from_non_custom_properties: NonCustomReferenceMap<Vec<Name>>,
    function_levels: FunctionLevels,
    initial_properties: ComputedCustomProperties,
    declarations: Vec<(&'a CustomDeclaration, CascadePriority, usize)>,
    substituted_keywords: std::collections::HashMap<usize, CSSWideKeyword>,
    replaying: bool,
}

/// The cascade levels of custom property values that call custom functions.
type FunctionLevels = PrecomputedHashMap<Name, CascadeLevel>;

/// Custom properties whose resolution waits for font-relative properties.
pub struct DeferredCustomProperties {
    values: CustomPropertiesMap,
    function_levels: FunctionLevels,
}

fn find_non_custom_references(
    registration: &PropertyRegistrationData,
    value: &VariableValue,
    may_have_color_scheme: bool,
    is_root_element: bool,
    include_universal: bool,
) -> Option<NonCustomReferences> {
    let dependent_types = registration.syntax.dependent_types();
    non_custom_dependencies(
        dependent_types,
        dependent_types.intersects(DependentDataTypes::LENGTH)
            || (include_universal && registration.syntax.is_universal()),
        value.references.non_custom_references,
        may_have_color_scheme,
        is_root_element,
    )
}

/// The non-custom references of a value that calls custom functions: those of the value and of
/// the reachable function bodies, which the functions' types or the registration may compute.
fn function_non_custom_references(
    dependencies: &functions::FunctionDependencies,
    registration: &PropertyRegistrationData,
    value: &VariableValue,
    may_have_color_scheme: bool,
    is_root_element: bool,
    include_universal: bool,
) -> Option<NonCustomReferences> {
    let dependent_types = dependencies.dependent_types | registration.syntax.dependent_types();
    non_custom_dependencies(
        dependent_types,
        dependent_types.intersects(DependentDataTypes::LENGTH)
            || (include_universal && registration.syntax.is_universal()),
        value.references.non_custom_references | dependencies.non_custom_references,
        may_have_color_scheme,
        is_root_element,
    )
}

fn non_custom_dependencies(
    dependent_types: DependentDataTypes,
    may_reference_length: bool,
    references: NonCustomReferences,
    may_have_color_scheme: bool,
    is_root_element: bool,
) -> Option<NonCustomReferences> {
    let mut value_dependencies = NonCustomReferences::empty();
    if dependent_types.intersects(DependentDataTypes::COLOR) {
        value_dependencies |= references & (NonCustomReferences::COLOR | NonCustomReferences::ACCENT_COLOR);
    }
    if may_reference_length {
        value_dependencies |= references.for_element(is_root_element);
    }
    if !value_dependencies.is_empty() {
        return Some(value_dependencies);
    }
    if dependent_types.intersects(DependentDataTypes::COLOR) && may_have_color_scheme {
        // NOTE(emilio): We might want to add a NonCustomReferences::COLOR_SCHEME or something but
        // it's not really needed for correctness, so for now we use an Option for that to signal
        // that there might be a dependencies.
        return Some(NonCustomReferences::empty());
    }
    None
}

impl<'a, 'b: 'a> CustomPropertiesBuilder<'a, 'b> {
    /// Create a new builder, inheriting from a given custom properties map.
    ///
    /// We expose this publicly mostly for @keyframe blocks.
    pub fn new_with_properties(
        stylist: &'a Stylist,
        custom_properties: ComputedCustomProperties,
        computed_context: &'a mut computed::Context<'b>,
    ) -> Self {
        Self {
            seen: PrecomputedHashSet::default(),
            reverted: Default::default(),
            reverted_rules: Default::default(),
            may_have_cycles: false,
            has_color_scheme: false,
            initial_properties: custom_properties.clone(),
            custom_properties,
            stylist,
            computed_context,
            references_from_non_custom_properties: NonCustomReferenceMap::default(),
            function_levels: FunctionLevels::default(),
            declarations: Vec::new(),
            substituted_keywords: Default::default(),
            replaying: false,
        }
    }

    /// Create a new builder, inheriting from the right style given context.
    pub fn new(stylist: &'a Stylist, context: &'a mut computed::Context<'b>) -> Self {
        let is_root_element = context.is_root_element();

        let inherited = context.inherited_custom_properties();
        let initial_values = stylist.get_custom_property_initial_values();
        let properties = ComputedCustomProperties {
            inherited: if is_root_element {
                debug_assert!(inherited.is_empty());
                initial_values.inherited.clone()
            } else {
                inherited.inherited.clone()
            },
            non_inherited: initial_values.non_inherited.clone(),
        };

        // Reuse flags from computing registered custom properties initial values, such as
        // whether they depend on viewport units.
        context
            .style()
            .add_flags(stylist.get_custom_property_initial_values_flags());
        Self::new_with_properties(stylist, properties, context)
    }

    /// Cascade a given custom property declaration.
    pub fn cascade(
        &mut self,
        declaration: &'a CustomDeclaration,
        priority: CascadePriority,
        rule_identity: usize,
        attribute_tracker: &mut AttributeTracker,
    ) {
        if !self.replaying {
            self.declarations.push((declaration, priority, rule_identity));
        }
        let CustomDeclaration {
            ref name,
            ref value,
        } = *declaration;

        if self
            .reverted_rules
            .get(name)
            .is_some_and(|rules| rules.contains_key(&rule_identity))
        {
            return;
        }

        if let Some(&(reverted_priority, is_origin_revert)) = self.reverted.get(&name) {
            if !reverted_priority.allows_when_reverted(&priority, is_origin_revert) {
                return;
            }
        }

        let resolved_keyword = self.substituted_keywords.get(&(declaration as *const _ as usize))
            .copied().map(CustomDeclarationValue::CSSWideKeyword).or_else(|| match value {
            CustomDeclarationValue::Unparsed(value)
                if value.references.any_function
                    && !value.references.any_var
                    && !value.references.any_attr =>
            {
                let dependencies = functions::FunctionDependencies::new(
                    value,
                    &self
                        .computed_context
                        .function_scope(priority.cascade_level()),
                );
                if dependencies.names.is_empty()
                    && dependencies.non_custom_references.is_empty()
                    && dependencies.dependent_types.is_empty()
                    && !dependencies.any_attr
                {
                    Substitutor::new(
                        &self.custom_properties,
                        self.stylist,
                        self.computed_context,
                        EnvironmentResolutionMode::ResolveLiveEnvironment,
                        attribute_tracker,
                        Some(priority.cascade_level()),
                    )
                    .substitute_value(value, None)
                    .ok()
                    .and_then(|value| functions::keyword(&value.css))
                    .map(CustomDeclarationValue::CSSWideKeyword)
                } else {
                    None
                }
            },
            _ => None,
        });
        let value = resolved_keyword.as_ref().unwrap_or(value);

        if matches!(
            value,
            CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::RevertRule)
        ) {
            self.reverted_rules
                .entry(name)
                .or_default()
                .insert(rule_identity, priority);
            return;
        }

        let was_already_present = !self.seen.insert(name);
        if was_already_present {
            return;
        }

        if !self.value_may_affect_style(name, value) {
            return;
        }

        let map = &mut self.custom_properties;
        let registration = self.stylist.get_custom_property_registration(&name);
        match value {
            CustomDeclarationValue::Unparsed(unparsed_value) => {
                // At this point of the cascade we're not guaranteed to have seen the color-scheme
                // declaration, so need to assume the worst. We could track all system color
                // keyword tokens + the light-dark() function, but that seems non-trivial /
                // probably overkill.
                let may_have_color_scheme = true;
                // Non-custom dependency is really relevant for registered custom properties
                // that require computed value of such dependencies.
                let has_dependency = unparsed_value.references.any_var
                    || unparsed_value.references.any_attr
                    || unparsed_value.references.any_function
                    || find_non_custom_references(
                        registration,
                        unparsed_value,
                        may_have_color_scheme,
                        self.computed_context.is_root_element(),
                        /* include_unregistered = */ false,
                    )
                    .is_some();
                // If the variable value has no references to other properties, perform
                // substitution here instead of forcing a full traversal in `substitute_all`
                // afterwards.
                if !has_dependency {
                    return substitute_references_if_needed_and_apply(
                        name,
                        unparsed_value,
                        None,
                        map,
                        self.stylist,
                        self.computed_context,
                        attribute_tracker,
                    );
                }
                if unparsed_value.references.any_function {
                    self.function_levels
                        .insert(name.clone(), priority.cascade_level());
                }
                self.may_have_cycles = true;
                let value = ComputedRegisteredValue::universal(Arc::clone(unparsed_value));
                map.insert(registration, name, value);
            },
            CustomDeclarationValue::Parsed(parsed_value) => {
                let value = parsed_value.to_computed_value(&self.computed_context);
                map.insert(registration, name, value);
            },
            CustomDeclarationValue::CSSWideKeyword(keyword) => match keyword {
                CSSWideKeyword::RevertRule => unreachable!(),
                CSSWideKeyword::RevertLayer | CSSWideKeyword::Revert => {
                    let origin_revert = matches!(keyword, CSSWideKeyword::Revert);
                    if !origin_revert
                        && self.reverted_rules.get(name).is_some_and(|rules| {
                            rules.values().any(|reverted_priority| {
                                priority.allows_when_reverted(reverted_priority, false)
                            })
                        })
                    {
                        // Reverting this layer would revisit a rule that already
                        // reverted to this declaration, so the cycle resolves to unset.
                        return;
                    }
                    self.seen.remove(name);
                    self.reverted.insert(name, (priority, origin_revert));
                },
                CSSWideKeyword::Initial => {
                    // For non-inherited custom properties, 'initial' was handled in value_may_affect_style.
                    debug_assert!(registration.inherits(), "Should've been handled earlier");
                    remove_and_insert_initial_value(name, registration, map);
                },
                CSSWideKeyword::Inherit => {
                    // For inherited custom properties, 'inherit' was handled in value_may_affect_style.
                    debug_assert!(!registration.inherits(), "Should've been handled earlier");
                    if let Some(inherited_value) = self
                        .computed_context
                        .inherited_custom_properties()
                        .non_inherited
                        .get(name)
                    {
                        map.insert(registration, name, inherited_value.clone());
                    }
                },
                // handled in value_may_affect_style
                CSSWideKeyword::Unset => unreachable!(),
            },
        }
    }

    /// Fast check to avoid calling maybe_note_non_custom_dependency in ~all cases.
    #[inline]
    pub fn might_have_non_custom_dependency(id: LonghandId, decl: &PropertyDeclaration) -> bool {
        if id == LonghandId::ColorScheme {
            return true;
        }
        if matches!(
            id,
            LonghandId::LineHeight
                | LonghandId::FontSize
                | LonghandId::Color
                | LonghandId::AccentColor
        ) {
            return matches!(decl, PropertyDeclaration::WithVariables(..));
        }
        false
    }

    /// Note a non-custom property with variable reference that may in turn depend on that property.
    /// e.g. `font-size` depending on a custom property that may be a registered property using `em`.
    /// `level` is the cascade level of the declaration.
    pub fn maybe_note_non_custom_dependency(
        &mut self,
        id: LonghandId,
        decl: &PropertyDeclaration,
        level: CascadeLevel,
    ) {
        debug_assert!(Self::might_have_non_custom_dependency(id, decl));
        if id == LonghandId::ColorScheme {
            // If we might change the color-scheme, we need to defer computation of colors.
            self.has_color_scheme = true;
            return;
        }

        let value = match decl {
            PropertyDeclaration::WithVariables(ref v) => &v.value.variable_value,
            _ => return,
        };
        let refs = &value.references;

        if !refs.any_var && !refs.any_attr && !refs.any_function {
            return;
        }

        // With unit algebra in `calc()`, references aren't limited to `font-size`.
        // For example, `--foo: 100ex; font-weight: calc(var(--foo) / 1ex);`,
        // or `--foo: 1em; zoom: calc(var(--foo) * 30px / 2em);`
        let references = match id {
            LonghandId::FontSize => {
                if self.computed_context.is_root_element() {
                    NonCustomReferences::ROOT_FONT_UNITS
                } else {
                    NonCustomReferences::FONT_UNITS
                }
            },
            LonghandId::LineHeight => {
                if self.computed_context.is_root_element() {
                    NonCustomReferences::ROOT_LH_UNITS | NonCustomReferences::ROOT_FONT_UNITS
                } else {
                    NonCustomReferences::LH_UNITS | NonCustomReferences::FONT_UNITS
                }
            },
            LonghandId::Color => NonCustomReferences::COLOR,
            LonghandId::AccentColor => NonCustomReferences::ACCENT_COLOR,
            _ => return,
        };

        let called = refs.any_function.then(|| {
            functions::FunctionDependencies::new(
                value,
                &self.computed_context.function_scope(level),
            )
            .names
        });
        let variables: Vec<Atom> = refs
            .refs
            .iter()
            .filter(|reference| !refs.in_lazy_argument(reference))
            .filter_map(|reference| reference.function.variable_name())
            .chain(called.iter().flatten())
            .filter(|name| {
                self.stylist
                    .get_custom_property_registration(name)
                    .syntax
                    .dependent_types()
                    .intersects(
                        if matches!(id, LonghandId::Color | LonghandId::AccentColor) {
                            DependentDataTypes::COLOR
                        } else {
                            DependentDataTypes::LENGTH
                        },
                    )
            })
            .cloned()
            .collect();
        references.for_each(|idx| {
            let entry = &mut self.references_from_non_custom_properties[idx];
            let was_none = entry.is_none();
            let v = entry.get_or_insert_with(|| variables.clone());
            if was_none {
                return;
            }
            v.extend(variables.iter().cloned());
        });
    }

    fn value_may_affect_style(&self, name: &Name, value: &CustomDeclarationValue) -> bool {
        let registration = self.stylist.get_custom_property_registration(&name);
        match *value {
            CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::Inherit) => {
                // For inherited custom properties, explicit 'inherit' means we
                // can just use any existing value in the inherited
                // CustomPropertiesMap.
                if registration.inherits() {
                    return false;
                }
            },
            CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::Initial) => {
                // For non-inherited custom properties, explicit 'initial' means
                // we can just use any initial value in the registration.
                if !registration.inherits() {
                    return false;
                }
            },
            CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::Unset) => {
                // Explicit 'unset' means we can either just use any existing
                // value in the inherited CustomPropertiesMap or the initial
                // value in the registration.
                return false;
            },
            _ => {},
        }

        let existing_value = self.custom_properties.get(registration, &name);
        let existing_value = match existing_value {
            None => {
                if matches!(
                    value,
                    CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::Initial)
                ) {
                    debug_assert!(registration.inherits(), "Should've been handled earlier");
                    // The initial value of a custom property without a
                    // guaranteed-invalid initial value is the same as it
                    // not existing in the map.
                    if registration.initial_value.is_none() {
                        return false;
                    }
                }
                return true;
            },
            Some(v) => v,
        };
        let computed_value = match value {
            CustomDeclarationValue::Unparsed(value) => {
                // Don't bother overwriting an existing value with the same
                // specified value.
                if let Some(existing_value) = existing_value.as_universal() {
                    return existing_value != value;
                }
                if !registration.syntax.is_universal() {
                    compute_value(
                        &value.css,
                        &value.url_data,
                        registration,
                        self.computed_context,
                        &format!("--{name}"),
                    )
                    .ok()
                } else {
                    None
                }
            },
            CustomDeclarationValue::Parsed(value) => {
                Some(value.to_computed_value(&self.computed_context))
            },
            CustomDeclarationValue::CSSWideKeyword(kw) => {
                match kw {
                    CSSWideKeyword::Inherit => {
                        debug_assert!(!registration.inherits(), "Should've been handled earlier");
                        // existing_value is the registered initial value.
                        // Don't bother adding it to self.custom_properties.non_inherited
                        // if the key is also absent from self.inherited.non_inherited.
                        if self
                            .computed_context
                            .inherited_custom_properties()
                            .non_inherited
                            .get(name)
                            .is_none()
                        {
                            return false;
                        }
                    },
                    CSSWideKeyword::Initial => {
                        debug_assert!(registration.inherits(), "Should've been handled earlier");
                        // Don't bother overwriting an existing value with the initial value specified in
                        // the registration.
                        if let Some(initial_value) = self
                            .stylist
                            .get_custom_property_initial_values()
                            .get(registration, name)
                        {
                            return existing_value != initial_value;
                        }
                    },
                    CSSWideKeyword::Unset => {
                        debug_assert!(false, "Should've been handled earlier");
                    },
                    CSSWideKeyword::Revert
                    | CSSWideKeyword::RevertLayer
                    | CSSWideKeyword::RevertRule => {},
                }
                None
            },
        };

        if let Some(value) = computed_value {
            return existing_value.v != value.v || existing_value.attr_tainted != value.attr_tainted;
        }

        true
    }

    fn resolve_substituted_reverts(&mut self, attribute_tracker: &mut AttributeTracker) {
        if !self.may_have_cycles {
            return;
        }
        let mut history = vec![self.substituted_keywords.clone()];
        loop {
            let mut keywords = std::collections::HashMap::new();
            for &(declaration, priority, _) in &self.declarations {
                let CustomDeclarationValue::Unparsed(value) = &declaration.value else {
                    continue;
                };
                if !value.has_references() {
                    continue;
                }
                let mut substitutor = Substitutor::new(
                    &self.custom_properties,
                    self.stylist,
                    self.computed_context,
                    EnvironmentResolutionMode::ResolveLiveEnvironment,
                    attribute_tracker,
                    Some(priority.cascade_level()),
                );
                let owner = substitutor.calls.guard_property(&declaration.name).unwrap();
                let keyword = substitutor
                    .substitute_value(value, None)
                    .ok()
                    .and_then(|value| functions::keyword(&value.css));
                if !substitutor.calls.release(owner) {
                    if let Some(
                        keyword @ (CSSWideKeyword::Revert
                        | CSSWideKeyword::RevertLayer
                        | CSSWideKeyword::RevertRule),
                    ) = keyword
                    {
                        keywords.insert(declaration as *const _ as usize, keyword);
                    }
                }
            }
            if keywords == self.substituted_keywords {
                return;
            }
            let cycle = history.iter().position(|previous| *previous == keywords);
            if let Some(start) = cycle {
                for previous in &history[start..] {
                    for key in previous.keys().chain(self.substituted_keywords.keys()) {
                        if previous.get(key) != self.substituted_keywords.get(key) {
                            keywords.insert(*key, CSSWideKeyword::Unset);
                        }
                    }
                }
            }
            self.substituted_keywords = keywords;
            self.custom_properties = self.initial_properties.clone();
            self.seen.clear();
            self.reverted.clear();
            self.reverted_rules.clear();
            self.function_levels.clear();
            self.may_have_cycles = false;
            self.replaying = true;
            for index in 0..self.declarations.len() {
                let (declaration, priority, rule) = self.declarations[index];
                self.cascade(declaration, priority, rule, attribute_tracker);
            }
            self.replaying = false;
            if cycle.is_some() {
                return;
            }
            history.push(self.substituted_keywords.clone());
        }
    }

    /// Computes the map of applicable custom properties, as well as
    /// longhand properties that are now considered invalid-at-compute time.
    /// The result is saved into the computed context.
    ///
    /// If there was any specified property or non-inherited custom property
    /// with an initial value, we've created a new map and now we
    /// need to remove any potential cycles (And marking non-custom
    /// properties), and wrap it in an arc.
    ///
    /// Some registered custom properties may require font-related properties
    /// be resolved to resolve. If these properties are not resolved at this time,
    /// `defer` should be set to `Yes`, which will leave such custom properties,
    /// and other properties referencing them, untouched. These properties are
    /// returned separately, to be resolved by `build_deferred` to fully resolve
    /// all custom properties after all necessary non-custom properties are resolved.
    pub fn build(
        mut self,
        defer: DeferFontRelativeCustomPropertyResolution,
        attribute_tracker: &mut AttributeTracker,
    ) -> Option<DeferredCustomProperties> {
        self.resolve_substituted_reverts(attribute_tracker);
        let mut deferred_custom_properties = None;
        if self.may_have_cycles {
            if defer == DeferFontRelativeCustomPropertyResolution::Yes {
                deferred_custom_properties = Some(CustomPropertiesMap::default());
            }
            let mut invalid_non_custom_properties = LonghandIdSet::default();
            substitute_all(
                &mut self.custom_properties,
                deferred_custom_properties.as_mut(),
                &mut invalid_non_custom_properties,
                self.has_color_scheme,
                &self.seen,
                &self.references_from_non_custom_properties,
                &self.function_levels,
                self.stylist,
                self.computed_context,
                attribute_tracker,
            );
            self.computed_context.builder.invalid_non_custom_properties =
                invalid_non_custom_properties;
        }

        self.custom_properties.shrink_to_fit();

        // Some pages apply a lot of redundant custom properties, see e.g.
        // bug 1758974 comment 5. Try to detect the case where the values
        // haven't really changed, and save some memory by reusing the inherited
        // map in that case.
        let initial_values = self.stylist.get_custom_property_initial_values();
        self.computed_context.builder.custom_properties = ComputedCustomProperties {
            inherited: if self
                .computed_context
                .inherited_custom_properties()
                .inherited
                == self.custom_properties.inherited
            {
                self.computed_context
                    .inherited_custom_properties()
                    .inherited
                    .clone()
            } else {
                self.custom_properties.inherited
            },
            non_inherited: if initial_values.non_inherited == self.custom_properties.non_inherited {
                initial_values.non_inherited.clone()
            } else {
                self.custom_properties.non_inherited
            },
        };

        deferred_custom_properties.map(|values| DeferredCustomProperties {
            values,
            function_levels: self.function_levels,
        })
    }

    /// Fully resolve all deferred custom properties, assuming that the incoming context
    /// has necessary properties resolved.
    pub fn build_deferred(
        deferred: DeferredCustomProperties,
        stylist: &Stylist,
        computed_context: &mut computed::Context,
        attribute_tracker: &mut AttributeTracker,
    ) {
        let DeferredCustomProperties {
            values: deferred,
            function_levels,
        } = deferred;
        if deferred.is_empty() {
            return;
        }
        let mut custom_properties = std::mem::take(&mut computed_context.builder.custom_properties);
        for (name, value) in deferred.iter() {
            if let Some(value) = value {
                custom_properties.insert(
                    stylist.get_custom_property_registration(name),
                    name,
                    value.clone(),
                );
            }
        }
        for (k, v) in deferred.iter() {
            let Some(v) = v else { continue };
            let Some(v) = v.as_universal() else {
                unreachable!("Computing should have been deferred!")
            };
            substitute_references_if_needed_and_apply(
                k,
                v,
                function_levels.get(k).copied(),
                &mut custom_properties,
                stylist,
                computed_context,
                attribute_tracker,
            );
        }
        computed_context.builder.custom_properties = custom_properties;
    }
}

/// Resolve all custom properties to either substituted, invalid, or unset
/// (meaning we should use the inherited value).
///
/// It does cycle dependencies removal at the same time as substitution.
fn substitute_all(
    custom_properties_map: &mut ComputedCustomProperties,
    mut deferred_properties_map: Option<&mut CustomPropertiesMap>,
    invalid_non_custom_properties: &mut LonghandIdSet,
    has_color_scheme: bool,
    seen: &PrecomputedHashSet<&Name>,
    references_from_non_custom_properties: &NonCustomReferenceMap<Vec<Name>>,
    function_levels: &FunctionLevels,
    stylist: &Stylist,
    computed_context: &computed::Context,
    attr_provider: &mut AttributeTracker,
) {
    // The cycle dependencies removal in this function is a variant
    // of Tarjan's algorithm. It is mostly based on the pseudo-code
    // listed in
    // https://en.wikipedia.org/w/index.php?
    // title=Tarjan%27s_strongly_connected_components_algorithm&oldid=801728495

    #[derive(Clone, Eq, PartialEq, Debug)]
    enum VarType {
        Custom(Name),
        NonCustom(SingleNonCustomReference),
    }

    /// Struct recording necessary information for each variable.
    #[derive(Debug)]
    struct VarInfo {
        /// The name of the variable. It will be taken to save addref
        /// when the corresponding variable is popped from the stack.
        /// This also serves as a mark for whether the variable is
        /// currently in the stack below.
        var: Option<VarType>,
        /// If the variable is in a dependency cycle, lowlink represents
        /// a smaller index which corresponds to a variable in the same
        /// strong connected component, which is known to be accessible
        /// from this variable. It is not necessarily the root, though.
        lowlink: usize,
    }
    /// Context struct for traversing the variable graph, so that we can
    /// avoid referencing all the fields multiple times.
    struct Context<'a, 'b: 'a> {
        /// Number of variables visited. This is used as the order index
        /// when we visit a new unresolved variable.
        count: usize,
        /// The map from custom property name to its order index.
        index_map: PrecomputedHashMap<Name, usize>,
        /// Mapping from a non-custom dependency to its order index.
        non_custom_index_map: NonCustomReferenceMap<usize>,
        /// Information of each variable indexed by the order index.
        var_info: SmallVec<[VarInfo; 5]>,
        /// The stack of order index of visited variables. It contains
        /// all unfinished strong connected components.
        stack: SmallVec<[usize; 5]>,
        /// References to non-custom properties in this strongly connected component.
        non_custom_references: NonCustomReferences,
        /// Whether the builder has seen a non-custom color-scheme reference.
        has_color_scheme: bool,
        /// Whether this strongly connected component contains any custom properties involving
        /// value computation.
        contains_computed_custom_property: bool,
        map: &'a mut ComputedCustomProperties,
        /// The stylist is used to get registered properties, and to resolve the environment to
        /// substitute `env()` variables.
        stylist: &'a Stylist,
        /// The computed context is used to get inherited custom
        /// properties  and compute registered custom properties.
        computed_context: &'a computed::Context<'b>,
        /// Longhand IDs that became invalid due to dependency cycle(s).
        invalid_non_custom_properties: &'a mut LonghandIdSet,
        /// Properties that cannot yet be substituted. Note we store both inherited and
        /// non-inherited properties in the same map, since we need to make sure we iterate through
        /// them in the right order.
        deferred_properties: Option<&'a mut CustomPropertiesMap>,
        /// The cascade levels of values that call custom functions.
        function_levels: &'a FunctionLevels,
    }

    /// This function combines the traversal for cycle removal and value
    /// substitution. It returns either a signal None if this variable
    /// has been fully resolved (to either having no reference or being
    /// marked invalid), or the order index for the given name.
    ///
    /// When it returns, the variable corresponds to the name would be
    /// in one of the following states:
    /// * It is still in context.stack, which means it is part of an
    ///   potentially incomplete dependency circle.
    /// * It has been removed from the map.  It can be either that the
    ///   substitution failed, or it is inside a dependency circle.
    ///   When this function removes a variable from the map because
    ///   of dependency circle, it would put all variables in the same
    ///   strong connected component to the set together.
    /// * It doesn't have any reference, because either this variable
    ///   doesn't have reference at all in specified value, or it has
    ///   been completely resolved.
    /// * There is no such variable at all.
    fn traverse<'a, 'b>(
        var: VarType,
        non_custom_references: &NonCustomReferenceMap<Vec<Name>>,
        context: &mut Context<'a, 'b>,
        attribute_tracker: &mut AttributeTracker,
    ) -> Option<usize> {
        // Some shortcut checks.
        let value = match var {
            VarType::Custom(ref name) => {
                let registration = context.stylist.get_custom_property_registration(name);
                let value = context.map.get(registration, name)?.as_universal()?;
                let random_value;
                let value = if value.references.refs.iter().any(|reference| matches!(reference.function, SubstitutionFunction::RandomItem { .. } | SubstitutionFunction::If(_))) {
                    let mut preflight = context.map.clone();
                    if let Some(deferred) = context.deferred_properties.as_deref() {
                        for (name, value) in deferred.iter() {
                            if let Some(value) = value { preflight.insert(context.stylist.get_custom_property_registration(name), name, value.clone()); }
                        }
                    }
                    let mut substitutor = Substitutor::new(&preflight, context.stylist, context.computed_context,
                        EnvironmentResolutionMode::ResolveLiveEnvironment, attribute_tracker, context.function_levels.get(name).copied());
                    let owner = substitutor.calls.guard_property(name).ok()?;
                    let _ = substitutor.substitute_value(value, None);
                    random_value = Arc::new(substitutor.selected_dependencies(value));
                    substitutor.calls.release(owner);
                    &random_value
                } else { value };
                let is_root = context.computed_context.is_root_element();
                // We need to keep track of potential non-custom-references even on unregistered
                // properties for cycle-detection purposes.
                let non_custom_refs = find_non_custom_references(
                    registration,
                    value,
                    context.has_color_scheme,
                    is_root,
                    /* include_unregistered = */ true,
                );
                context.non_custom_references |= non_custom_refs.unwrap_or_default();
                let has_dependency = value.references.any_var
                    || value.references.any_attr
                    || value.references.any_function
                    || non_custom_refs.is_some();
                // Nothing to resolve.
                if !has_dependency {
                    debug_assert!(!value.references.any_env, "Should've been handled earlier");
                    if !registration.syntax.is_universal() {
                        // We might still need to compute the value if this is not an universal
                        // registration if we thought this had a dependency before but turned out
                        // not to be (due to has_color_scheme, for example). Note that if this was
                        // already computed we would've bailed out in the as_universal() check.
                        debug_assert!(
                            registration
                                .syntax
                                .dependent_types()
                                .intersects(DependentDataTypes::COLOR),
                            "How did an unresolved value get here otherwise?",
                        );
                        let value = value.clone();
                        substitute_references_if_needed_and_apply(
                            name,
                            &value,
                            None,
                            &mut context.map,
                            context.stylist,
                            context.computed_context,
                            attribute_tracker,
                        );
                    }
                    return None;
                }

                // Has this variable been visited?
                match context.index_map.entry(name.clone()) {
                    Entry::Occupied(entry) => {
                        return Some(*entry.get());
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(context.count);
                    },
                }
                context.contains_computed_custom_property |= !registration.syntax.is_universal();

                let dependencies = value.references.any_function.then(|| {
                    let scope = context
                        .computed_context
                        .function_scope(context.function_levels[name]);
                    functions::FunctionDependencies::new(value, &scope)
                });
                let function_references = dependencies.as_ref().and_then(|dependencies| {
                    function_non_custom_references(
                        dependencies,
                        registration,
                        value,
                        context.has_color_scheme,
                        is_root,
                        /* include_universal = */ true,
                    )
                });
                context.non_custom_references |= function_references.unwrap_or_default();

                // Hold a strong reference to the value so that we don't
                // need to keep reference to context.map.
                Some((
                    value.clone(),
                    dependencies,
                    function_references.unwrap_or_default(),
                ))
            },
            VarType::NonCustom(ref non_custom) => {
                let entry = &mut context.non_custom_index_map[*non_custom];
                if let Some(v) = entry {
                    return Some(*v);
                }
                *entry = Some(context.count);
                None
            },
        };

        // Add new entry to the information table.
        let index = context.count;
        context.count += 1;
        debug_assert_eq!(index, context.var_info.len());
        context.var_info.push(VarInfo {
            var: Some(var.clone()),
            lowlink: index,
        });
        context.stack.push(index);

        let mut self_ref = false;
        let mut lowlink = index;
        let visit_link = |var: VarType,
                          context: &mut Context,
                          lowlink: &mut usize,
                          self_ref: &mut bool,
                          attribute_tracker: &mut AttributeTracker| {
            let next_index = match traverse(var, non_custom_references, context, attribute_tracker)
            {
                Some(index) => index,
                // There is nothing to do if the next variable has been
                // fully resolved at this point.
                None => {
                    return;
                },
            };
            let next_info = &context.var_info[next_index];
            if next_index > index {
                // The next variable has a larger index than us, so it
                // must be inserted in the recursive call above. We want
                // to get its lowlink.
                *lowlink = cmp::min(*lowlink, next_info.lowlink);
            } else if next_index == index {
                *self_ref = true;
            } else if next_info.var.is_some() {
                // The next variable has a smaller order index and it is
                // in the stack, so we are at the same component.
                *lowlink = cmp::min(*lowlink, next_index);
            }
        };
        if let Some((ref v, ref dependencies, function_references)) = value {
            debug_assert!(
                matches!(var, VarType::Custom(_)),
                "Non-custom property has references?"
            );

            let reached = {
                let mut substitutor = Substitutor::new(
                    context.map,
                    context.stylist,
                    context.computed_context,
                    EnvironmentResolutionMode::ResolveLiveEnvironment,
                    attribute_tracker,
                    context
                        .function_levels
                        .get(match &var {
                            VarType::Custom(name) => name,
                            VarType::NonCustom(_) => unreachable!(),
                        })
                        .copied(),
                );
                substitutor.dependency_names = Some(Vec::new());
                let owner = match &var {
                    VarType::Custom(name) => substitutor.calls.guard_property(name).ok(),
                    VarType::NonCustom(_) => None,
                };
                let _ = substitutor.substitute_value(v, None);
                if let Some(owner) = owner {
                    substitutor.calls.release(owner);
                }
                substitutor.dependency_names.take().unwrap()
            };
            for name in reached {
                visit_link(
                    VarType::Custom(name),
                    context,
                    &mut lowlink,
                    &mut self_ref,
                    attribute_tracker,
                );
            }

            // ... and the custom properties that called functions read...
            for name in dependencies
                .iter()
                .flat_map(|dependencies| &dependencies.names)
            {
                visit_link(
                    VarType::Custom(name.clone()),
                    context,
                    &mut lowlink,
                    &mut self_ref,
                    attribute_tracker,
                );
            }

            for name in conditionals::condition_names(v) {
                if context
                    .index_map
                    .get(&name)
                    .is_some_and(|index| context.var_info[*index].var.is_some())
                {
                    continue;
                }
                visit_link(
                    VarType::Custom(name),
                    context,
                    &mut lowlink,
                    &mut self_ref,
                    attribute_tracker,
                );
            }

            // ... Then non-custom properties.
            (v.references.non_custom_references | function_references).for_each(|r| {
                visit_link(
                    VarType::NonCustom(r),
                    context,
                    &mut lowlink,
                    &mut self_ref,
                    attribute_tracker,
                );
            });
        } else if let VarType::NonCustom(non_custom) = var {
            let entry = &non_custom_references[non_custom];
            if let Some(deps) = entry.as_ref() {
                for d in deps {
                    // Visit any reference from this non-custom property to custom properties.
                    visit_link(
                        VarType::Custom(d.clone()),
                        context,
                        &mut lowlink,
                        &mut self_ref,
                        attribute_tracker,
                    );
                }
            }
        }

        context.var_info[index].lowlink = lowlink;
        if lowlink != index {
            // This variable is in a loop, but it is not the root of
            // this strong connected component. We simply return for
            // now, and the root would remove it from the map.
            //
            // This cannot be removed from the map here, because
            // otherwise the shortcut check at the beginning of this
            // function would return the wrong value.
            return Some(index);
        }

        // This is the root of a strong-connected component.
        let mut in_loop = self_ref;
        let name;

        let handle_variable_in_loop = |name: &Name, context: &mut Context<'a, 'b>| {
            if context.contains_computed_custom_property {
                // These non-custom properties can't become invalid-at-compute-time from
                // cyclic dependencies purely consisting of non-registered properties.
                if context.non_custom_references.intersects(
                    NonCustomReferences::FONT_UNITS | NonCustomReferences::ROOT_FONT_UNITS,
                ) {
                    context
                        .invalid_non_custom_properties
                        .insert(LonghandId::FontSize);
                }
                if context
                    .non_custom_references
                    .intersects(NonCustomReferences::LH_UNITS | NonCustomReferences::ROOT_LH_UNITS)
                {
                    context
                        .invalid_non_custom_properties
                        .insert(LonghandId::LineHeight);
                }
                if context
                    .non_custom_references
                    .contains(NonCustomReferences::COLOR)
                {
                    context
                        .invalid_non_custom_properties
                        .insert(LonghandId::Color);
                }
                if context
                    .non_custom_references
                    .contains(NonCustomReferences::ACCENT_COLOR)
                {
                    context
                        .invalid_non_custom_properties
                        .insert(LonghandId::AccentColor);
                }
            }
            // This variable is in loop. Resolve to invalid.
            handle_invalid_at_computed_value_time(name, context.map, context.computed_context);
        };
        loop {
            let var_index = context
                .stack
                .pop()
                .expect("The current variable should still be in stack");
            let var_info = &mut context.var_info[var_index];
            // We should never visit the variable again, so it's safe
            // to take the name away, so that we don't do additional
            // reference count.
            let var_name = var_info
                .var
                .take()
                .expect("Variable should not be poped from stack twice");
            if var_index == index {
                name = match var_name {
                    VarType::Custom(name) => name,
                    // At the root of this component, and it's a non-custom
                    // reference - we have nothing to substitute, so
                    // it's effectively resolved.
                    VarType::NonCustom(..) => return None,
                };
                break;
            }
            if let VarType::Custom(name) = var_name {
                // Anything here is in a loop which can traverse to the
                // variable we are handling, so it's invalid at
                // computed-value time.
                handle_variable_in_loop(&name, context);
            }
            in_loop = true;
        }
        // We've gotten to the root of this strongly connected component, so clear
        // whether or not it involved non-custom references.
        // It's fine to track it like this, because non-custom properties currently
        // being tracked can only participate in any loop only once.
        if in_loop {
            handle_variable_in_loop(&name, context);
            context.non_custom_references = NonCustomReferences::default();
            return None;
        }

        if let Some((ref v, ref dependencies, _)) = value {
            let registration = context.stylist.get_custom_property_registration(&name);

            let mut defer = false;
            if let Some(ref mut deferred) = context.deferred_properties {
                // We need to defer this property if it has a non-custom property dependency, or
                // any variable that it references is already deferred.
                defer = find_non_custom_references(
                    registration,
                    v,
                    context.has_color_scheme,
                    context.computed_context.is_root_element(),
                    /* include_unregistered = */ false,
                )
                .is_some()
                    || v.references.refs.iter().any(|reference| {
                        reference
                            .function
                            .variable_name()
                            .is_some_and(|name| deferred.get(name).is_some())
                            || reference.function.is_attr()
                    })
                    || conditionals::condition_names(v).iter()
                        .any(|name| deferred.get(name).is_some())
                    || dependencies.as_ref().is_some_and(|dependencies| {
                        dependencies.any_attr
                            || dependencies
                                .names
                                .iter()
                                .any(|name| deferred.get(name).is_some())
                            || function_non_custom_references(
                                dependencies,
                                registration,
                                v,
                                context.has_color_scheme,
                                context.computed_context.is_root_element(),
                                /* include_universal = */ false,
                            )
                            .is_some()
                    });

                if defer {
                    let value = ComputedRegisteredValue::universal(Arc::clone(v));
                    deferred.insert(&name, value);
                    context.map.remove(registration, &name);
                }
            }

            // If there are no var references we should already be computed and substituted by now.
            if !defer
                && (v.references.any_var || v.references.any_attr || v.references.any_function)
            {
                substitute_references_if_needed_and_apply(
                    &name,
                    v,
                    context.function_levels.get(&name).copied(),
                    &mut context.map,
                    context.stylist,
                    context.computed_context,
                    attribute_tracker,
                );
            }
        }
        context.non_custom_references = NonCustomReferences::default();

        // All resolved, so return the signal value.
        None
    }

    // Note that `seen` doesn't contain names inherited from our parent, but
    // those can't have variable references (since we inherit the computed
    // variables) so we don't want to spend cycles traversing them anyway.
    for name in seen {
        let mut context = Context {
            count: 0,
            index_map: PrecomputedHashMap::default(),
            non_custom_index_map: NonCustomReferenceMap::default(),
            stack: SmallVec::new(),
            var_info: SmallVec::new(),
            map: custom_properties_map,
            non_custom_references: NonCustomReferences::default(),
            has_color_scheme,
            stylist,
            computed_context,
            invalid_non_custom_properties,
            deferred_properties: deferred_properties_map.as_deref_mut(),
            contains_computed_custom_property: false,
            function_levels,
        };
        traverse(
            VarType::Custom((*name).clone()),
            references_from_non_custom_properties,
            &mut context,
            attr_provider,
        );
    }
}

// See https://drafts.csswg.org/css-variables-2/#invalid-at-computed-value-time
fn handle_invalid_at_computed_value_time(
    name: &Name,
    custom_properties: &mut ComputedCustomProperties,
    computed_context: &computed::Context,
) {
    let stylist = computed_context.style().stylist.unwrap();
    let registration = stylist.get_custom_property_registration(&name);
    if !registration.syntax.is_universal() {
        // For the root element, inherited maps are empty. We should just
        // use the initial value if any, rather than removing the name.
        if registration.inherits() && !computed_context.is_root_element() {
            let inherited = computed_context.inherited_custom_properties();
            if let Some(value) = inherited.get(registration, name) {
                custom_properties.insert(registration, name, value.clone());
                return;
            }
        } else if let Some(ref initial_value) = registration.initial_value {
            if let Ok(initial_value) = compute_value(
                &initial_value.css,
                &initial_value.url_data,
                registration,
                computed_context,
                &format!("--{name}"),
            ) {
                custom_properties.insert(registration, name, initial_value);
                return;
            }
        }
    }
    custom_properties.remove(registration, name);
}

/// Replace `var()`, `env()`, `attr()` and custom function calls in a pre-existing variable
/// value declared at `level`.
fn substitute_references_if_needed_and_apply(
    name: &Name,
    value: &Arc<VariableValue>,
    level: Option<CascadeLevel>,
    custom_properties: &mut ComputedCustomProperties,
    stylist: &Stylist,
    computed_context: &computed::Context,
    attribute_tracker: &mut AttributeTracker,
) {
    let registration = stylist.get_custom_property_registration(&name);
    if !value.has_references() && registration.syntax.is_universal() {
        // Trivial path: no references and no need to compute the value, just apply it directly.
        let computed_value = ComputedRegisteredValue::universal(Arc::clone(value));
        custom_properties.insert(registration, name, computed_value);
        return;
    }

    let inherited = computed_context.inherited_custom_properties();
    let url_data = &value.url_data;
    let mut substitutor = Substitutor::new(
        custom_properties,
        stylist,
        computed_context,
        EnvironmentResolutionMode::ResolveLiveEnvironment,
        attribute_tracker,
        level,
    );
    let owner = substitutor.calls.guard_property(name).unwrap();
    let substitution = substitutor.substitute_value(value, None);
    let substitution = if substitutor.calls.release(owner) {
        Err(())
    } else {
        substitution
    };
    let substitution = match substitution {
        Ok(v) => v,
        Err(..) => {
            handle_invalid_at_computed_value_time(name, custom_properties, computed_context);
            return;
        },
    };

    // If variable fallback results in a wide keyword, deal with it now.
    {
        let css = &substitution.css;
        let css_wide_kw = {
            let mut input = ParserInput::new(&css);
            let mut input = Parser::new(&mut input);
            CSSWideKeyword::parse(&mut input)
        };

        if let Ok(kw) = css_wide_kw {
            // TODO: It's unclear what this should do for revert / revert-layer, see
            // https://github.com/w3c/csswg-drafts/issues/9131. For now treating as unset
            // seems fine?
            match (
                kw,
                registration.inherits(),
                computed_context.is_root_element(),
            ) {
                (CSSWideKeyword::Initial, _, _)
                | (CSSWideKeyword::Revert, false, _)
                | (CSSWideKeyword::RevertLayer, false, _)
                | (CSSWideKeyword::RevertRule, false, _)
                | (CSSWideKeyword::Unset, false, _)
                | (CSSWideKeyword::Revert, true, true)
                | (CSSWideKeyword::RevertLayer, true, true)
                | (CSSWideKeyword::RevertRule, true, true)
                | (CSSWideKeyword::Unset, true, true)
                | (CSSWideKeyword::Inherit, _, true) => {
                    remove_and_insert_initial_value(name, registration, custom_properties);
                },
                (CSSWideKeyword::Revert, true, false)
                | (CSSWideKeyword::RevertLayer, true, false)
                | (CSSWideKeyword::RevertRule, true, false)
                | (CSSWideKeyword::Inherit, _, false)
                | (CSSWideKeyword::Unset, true, false) => {
                    match inherited.get(registration, name) {
                        Some(value) => {
                            custom_properties.insert(registration, name, value.clone());
                        },
                        None => {
                            custom_properties.remove(registration, name);
                        },
                    };
                },
            }
            return;
        }
    }

    let value = match substitution.into_value(url_data, registration, computed_context, name) {
        Ok(v) => v,
        Err(()) => {
            handle_invalid_at_computed_value_time(name, custom_properties, computed_context);
            return;
        },
    };

    custom_properties.insert(registration, name, value);
}

#[derive(Default)]
struct Substitution<'a> {
    css: Cow<'a, str>,
    first_token_type: TokenSerializationType,
    last_token_type: TokenSerializationType,
    attr_tainted: ThinVec<AttrTaintedRange>,
}

impl<'a> Substitution<'a> {
    fn from_value(v: VariableValue) -> Self {
        Substitution {
            css: v.css.into(),
            first_token_type: v.first_token_type,
            last_token_type: v.last_token_type,
            attr_tainted: v.attr_tainted,
        }
    }

    /// Mark the whole substitution as produced by `attr()`.
    fn attr_tainted(mut self) -> Self {
        self.attr_tainted = ThinVec::from([AttrTaintedRange {
            start: 0,
            end: self.css.len(),
        }]);
        self
    }

    fn into_value(
        self,
        url_data: &UrlExtraData,
        registration: &PropertyRegistrationData,
        computed_context: &computed::Context,
        name: &Name,
    ) -> Result<ComputedRegisteredValue, ()> {
        if registration.syntax.is_universal() {
            return Ok(ComputedRegisteredValue::universal(Arc::new(
                VariableValue {
                    css: self.css.into_owned(),
                    first_token_type: self.first_token_type,
                    last_token_type: self.last_token_type,
                    url_data: url_data.clone(),
                    references: Default::default(),
                    attr_tainted: self.attr_tainted,
                    private_names: ThinVec::new(),
                },
            )));
        }
        compute_value(&self.css, url_data, registration, computed_context, &format!("--{name}"))
            .and_then(|value| {
                let tainted = !self.attr_tainted.is_empty();
                if tainted && value.contains_url() { return Err(()); }
                Ok(value.with_attr_taint(tainted))
            })
    }

    fn new(
        css: Cow<'a, str>,
        first_token_type: TokenSerializationType,
        last_token_type: TokenSerializationType,
    ) -> Self {
        Self {
            css,
            first_token_type,
            last_token_type,
            attr_tainted: ThinVec::new(),
        }
    }

    fn into_owned(self) -> Substitution<'static> {
        Substitution {
            css: Cow::Owned(self.css.into_owned()),
            first_token_type: self.first_token_type,
            last_token_type: self.last_token_type,
            attr_tainted: self.attr_tainted,
        }
    }
}

fn compute_value(
    css: &str,
    url_data: &UrlExtraData,
    registration: &PropertyRegistrationData,
    computed_context: &computed::Context,
    property: &str,
) -> Result<ComputedRegisteredValue, ()> {
    debug_assert!(!registration.syntax.is_universal());

    let mut input = ParserInput::new(&css);
    let mut input = Parser::new(&mut input);

    SpecifiedRegisteredValue::compute_with_property(
        &mut input,
        registration,
        url_data,
        computed_context,
        AllowComputationallyDependent::Yes,
        property,
    )
}

/// Removes the named registered custom property and inserts its uncomputed initial value.
fn remove_and_insert_initial_value(
    name: &Name,
    registration: &PropertyRegistrationData,
    custom_properties: &mut ComputedCustomProperties,
) {
    custom_properties.remove(registration, name);
    if let Some(ref initial_value) = registration.initial_value {
        let value = ComputedRegisteredValue::universal(Arc::clone(initial_value));
        custom_properties.insert(registration, name, value);
    }
}

#[derive(Default)]
struct EarlyRandomState {
    count: usize,
    items: std::collections::HashMap<(bool, usize), (usize, bool)>,
}

/// Replaces arbitrary substitution functions in values of one declaration.
struct Substitutor<'a, 'b, 't> {
    custom_properties: &'a ComputedCustomProperties,
    stylist: &'a Stylist,
    computed_context: &'a computed::Context<'b>,
    environment_resolution: EnvironmentResolutionMode,
    attribute_tracker: &'a mut AttributeTracker<'t>,
    calls: functions::FunctionCalls<'a>,
    private_names: ThinVec<(Name, Name)>,
    dependency_names: Option<Vec<Name>>,
    random_property: Option<Atom>,
    random_source_offset: usize,
    early_random: std::collections::HashMap<(Atom, Vec<Atom>), EarlyRandomState>,
    conditional_branches: std::collections::HashMap<(Atom, Vec<Atom>, usize), Option<usize>>,
}

type ReferenceIter<'a> = std::iter::Peekable<std::slice::Iter<'a, SubstitutionFunctionReference>>;

impl<'a, 'b, 't> Substitutor<'a, 'b, 't> {
    fn new(
        custom_properties: &'a ComputedCustomProperties,
        stylist: &'a Stylist,
        computed_context: &'a computed::Context<'b>,
        environment_resolution: EnvironmentResolutionMode,
        attribute_tracker: &'a mut AttributeTracker<'t>,
        level: Option<CascadeLevel>,
    ) -> Self {
        Self {
            custom_properties,
            stylist,
            computed_context,
            environment_resolution,
            attribute_tracker,
            calls: functions::FunctionCalls::new(level),
            private_names: ThinVec::new(),
            dependency_names: None,
            random_property: None,
            random_source_offset: 0,
            early_random: Default::default(),
            conditional_branches: Default::default(),
        }
    }

    /// Replace the substitution functions of `value`, with `var()` seen from `frame`.
    fn substitute_value<'v>(
        &mut self,
        value: &'v VariableValue,
        frame: Option<usize>,
    ) -> Result<Substitution<'v>, ()>
    where
        'a: 'v,
    {
        let previous = std::mem::replace(&mut self.private_names, value.private_names.clone());
        let previous_property = self.random_property.clone();
        let previous_source = self.random_source_offset;
        if let Some(property) = &value.references.random_property {
            self.random_property = Some(property.clone());
            self.random_source_offset = 0;
        } else {
            self.random_source_offset += value.references.random_source_offset;
        }
        let mut references = value.references.refs.iter().peekable();
        let result = self.chunk(
            &value.css,
            0,
            value.css.len(),
            value.first_token_type,
            value.last_token_type,
            &value.url_data,
            frame,
            &mut references,
        );
        self.private_names = previous;
        self.random_property = previous_property;
        self.random_source_offset = previous_source;
        result
    }

    /// The value of `var(name)` seen from `frame`.
    fn variable(&mut self, frame: Option<usize>, name: &Name) -> Option<Substitution<'a>> {
        if let Some(value) = self.frame_variable(frame, name) {
            return value.map(Substitution::from_value);
        }
        let name = self
            .private_names
            .iter()
            .find(|(original, _)| original == name)
            .map_or_else(|| name.clone(), |(_, private)| private.clone());
        if let Some(dependencies) = &mut self.dependency_names {
            if !dependencies.contains(&name) { dependencies.push(name.clone()); }
        }
        let entry = self.calls.guard_property(&name).ok()?;
        let result = (|| {
            let registration = self.stylist.get_custom_property_registration(&name);
            let value = self.custom_properties.get(registration, &name)?;
            if let Some(value) = value.as_universal().filter(|value| value.has_references()) {
                return self
                    .substitute_value(value, frame)
                    .ok()
                    .and_then(|result| self.typed(registration, result, &value.url_data, &format!("--{name}"), None))
                    .map(Substitution::from_value);
            }
            Some(Substitution::from_value(value.to_variable_value()))
        })();
        if self.calls.release(entry) {
            None
        } else {
            result
        }
    }

    fn query_variable(&mut self, frame: Option<usize>, name: &Name) -> Option<Substitution<'a>> {
        if let Some(value) = self.frame_variable(frame, name) {
            return value.map(Substitution::from_value);
        }
        let name = self
            .private_names
            .iter()
            .find(|(original, _)| original == name)
            .map_or_else(|| name.clone(), |(_, private)| private.clone());
        if self.calls.property_is_guarded(&name) {
            return None;
        }
        self.variable(frame, &name)
    }

    fn chunk<'v>(
        &mut self,
        css: &'v str,
        start: usize,
        end: usize,
        first_token_type: TokenSerializationType,
        last_token_type: TokenSerializationType,
        url_data: &UrlExtraData,
        frame: Option<usize>,
        references: &mut ReferenceIter<'v>,
    ) -> Result<Substitution<'v>, ()>
    where
        'a: 'v,
    {
        if start == end {
            // Empty string. Easy.
            return Ok(Substitution::default());
        }
        // Easy case: no references involved.
        if references
            .peek()
            .map_or(true, |reference| reference.end > end)
        {
            let result = &css[start..end];
            return Ok(Substitution::new(
                Cow::Borrowed(result),
                first_token_type,
                last_token_type,
            ));
        }

        let mut substituted = ComputedValue::empty(url_data);
        let mut next_token_type = first_token_type;
        let mut cur_pos = start;
        while let Some(reference) = references.next_if(|reference| reference.end <= end) {
            if reference.start != cur_pos {
                substituted.push(
                    &css[cur_pos..reference.start],
                    next_token_type,
                    reference.prev_token_type,
                    &[],
                )?;
            }

            let substitution = self.reference(css, url_data, reference, frame, references)?;

            // Optimize the property: var(--...) case to avoid allocating at all.
            if reference.start == start && reference.end == end {
                return Ok(substitution);
            }

            substituted.push(
                &substitution.css,
                substitution.first_token_type,
                substitution.last_token_type,
                &substitution.attr_tainted,
            )?;
            next_token_type = reference.next_token_type;
            cur_pos = reference.end;
        }
        // Push the rest of the value if needed.
        if cur_pos != end {
            substituted.push(&css[cur_pos..end], next_token_type, last_token_type, &[])?;
        }
        Ok(Substitution::from_value(substituted))
    }

    fn level_for(&self, frame: Option<usize>) -> CascadeLevel {
        self.calls.level_for(frame, self.computed_context.scope)
    }

    fn early_random_property(&self) -> Atom {
        self.random_property.as_ref().map_or_else(|| Atom::from(""), |property| Atom::from(format!("early-{property}")))
    }

    fn early_random_scope(&self, property: &Atom, frame: Option<usize>) -> (Atom, Vec<Atom>) {
        (property.clone(), self.calls.random_scope(frame).unwrap_or_default().to_vec())
    }

    fn reserve_early_random(&mut self, property: &Atom, frame: Option<usize>, count: usize) -> usize {
        let scope = self.early_random_scope(property, frame);
        let state = self.early_random.entry(scope).or_default();
        let offset = state.count;
        state.count += count;
        offset
    }

    fn conditional_key(&self, slot: usize, frame: Option<usize>) -> (Atom, Vec<Atom>, usize) {
        let (property, scope) = self.early_random_scope(&self.early_random_property(), frame);
        (property, scope, self.random_source_offset + slot)
    }

    fn selected_dependencies(&mut self, value: &VariableValue) -> VariableValue {
        self.private_names = value.private_names.clone();
        self.random_property = value.references.random_property.clone();
        let mut resolved = value.clone();
        let mut excluded = Vec::new();
        for (index, reference) in value.references.refs.iter().enumerate() {
            if excluded.iter().any(|&(start, end)| reference.start >= start && reference.end <= end) { continue; }
            if let SubstitutionFunction::If(branches) = &reference.function {
                let key = self.conditional_key(reference.start, None);
                let selected = self.conditional_branches.get(&key).copied().flatten();
                if let SubstitutionFunction::If(resolved_branches) = &mut resolved.references.refs[index].function {
                    for (index, branch) in resolved_branches.iter_mut().enumerate() {
                        branch.selected = Some(selected == Some(index));
                        branch.evaluated = selected.map_or(true, |selected| index <= selected);
                    }
                }
                excluded.extend(branches.iter().enumerate().filter(|(index, _)| selected != Some(*index)).map(|(_, branch)| (branch.value.start, branch.value.end)));
                continue;
            }
            let SubstitutionFunction::RandomItem { arguments, identity, .. } = &reference.function else { continue; };
            let mut nested = value.references.refs[index + 1..].iter().peekable();
            let selected = self.random_item_option(&value.css, &value.url_data, arguments, identity.as_ref(), reference.start, None, &mut nested).ok().map(|(index, _)| index);
            if let SubstitutionFunction::RandomItem { selected_option, .. } = &mut resolved.references.refs[index].function { *selected_option = selected; }
            for (index, argument) in arguments.iter().skip(1).enumerate() {
                if selected != Some(index) { excluded.push((argument.start, argument.end)); }
            }
        }
        resolved.references.non_custom_references = resolved.references.eager_non_custom_references();
        resolved
    }

    fn random_item_option<'v>(
        &mut self, css: &'v str, url_data: &UrlExtraData, arguments: &[FunctionArgument],
        identity: Option<&crate::values::random::RandomFunctionIdentity>, slot: usize, frame: Option<usize>,
        references: &mut ReferenceIter<'v>,
    ) -> Result<(usize, bool), ()> where 'a: 'v {
                if self.computed_context.in_media_query || self.computed_context.in_container_query { return Err(()); }
                let [key, options @ ..] = arguments else { return Err(()); };
                if options.is_empty() { return Err(()); }
                let property = identity.filter(|identity| identity.is_bound()).map(|identity| identity.property.clone()).unwrap_or_else(|| self.early_random_property());
                let scope = self.early_random_scope(&property, frame);
                let slot = identity.filter(|identity| identity.is_bound()).map_or((false, self.random_source_offset + slot), |identity| (true, identity.index));
                if let Some(cached) = self.early_random.get(&scope).and_then(|state| state.items.get(&slot)) { return Ok(*cached); }
                let index = self.reserve_early_random(&property, frame, 1) + 1;
                let identity = crate::values::random::RandomFunctionIdentity { property: property.clone(), index };
                let key = self.chunk(css, key.start, key.end, key.first_token_type, key.last_token_type, url_data, frame, references)?;
                let tainted = !key.attr_tainted.is_empty();
                let context = crate::parser::ParserContext::new(
                    crate::stylesheets::Origin::Author,
                    url_data,
                    Some(crate::stylesheets::CssRuleType::Style),
                    style_traits::ParsingMode::DEFAULT,
                    self.computed_context.quirks_mode,
                    Default::default(), None, None,
                );
                let mut source = ParserInput::new(&key.css);
                let mut input = Parser::new(&mut source);
                let offset = self.reserve_early_random(&property, frame, 0);
                let context = context.for_random_declaration_with_offset(&property, offset, &mut input);
                self.reserve_early_random(&property, frame, context.random_function_count());
                let key = input.parse_entirely(|input| {
                    crate::values::random::RandomKey::parse_with_identity(
                        &identity,
                        input,
                        |input| crate::values::specified::Number::parse(&context, input),
                    )
                }).map_err(|_| ())?;
                let base = match key {
                    crate::values::random::RandomKey::Fixed(value) => {
                        let scope = self.calls.random_scope(frame).unwrap_or_default().to_vec();
                        let previous = self.computed_context.replace_random_function_scope(scope);
                        let value = f64::from(value.to_computed_value(self.computed_context));
                        let _ = self.computed_context.replace_random_function_scope(previous);
                        crate::values::random::normalized_base(value).ok_or(())?
                    },
                    crate::values::random::RandomKey::Cache(key) => self.calls.random_scope(frame).map_or_else(
                        || self.computed_context.resolve_random_base(&key),
                        |scope| self.computed_context.resolve_random_base_for_function(&key, scope),
                    ),
                };
                let index = ((base * options.len() as f64).floor() as usize).min(options.len() - 1);
                self.early_random.entry(scope).or_default().items.insert(slot, (index, tainted));
                Ok((index, tainted))

    }

    fn reference<'v>(
        &mut self,
        css: &'v str,
        url_data: &UrlExtraData,
        reference: &'v SubstitutionFunctionReference,
        frame: Option<usize>,
        references: &mut ReferenceIter<'v>,
    ) -> Result<Substitution<'v>, ()>
    where
        'a: 'v,
    {
        let simple_subst = |s: &str| {
            Some(Substitution::new(
                Cow::Owned(quoted_css_string(s)),
                TokenSerializationType::Nothing,
                TokenSerializationType::Nothing,
            ))
        };
        let substitution: Option<_> = match &reference.function {
            SubstitutionFunction::Var(name) => self.variable(frame, name),
            SubstitutionFunction::DynamicVar { name, arguments } => {
                let [argument, fallback @ ..] = &**arguments else {
                    return Err(());
                };
                let substituted = self.chunk(
                    css,
                    argument.start,
                    argument.end,
                    argument.first_token_type,
                    argument.last_token_type,
                    url_data,
                    frame,
                    references,
                ).ok();
                while references.next_if(|reference| reference.end <= argument.end).is_some() {}
                let tainted = substituted.as_ref().is_some_and(|value| !value.attr_tainted.is_empty());
                let value = name
                    .clone()
                    .or_else(|| substituted.as_ref().and_then(|value| parse_property_name_css(&value.css)))
                    .and_then(|name| self.variable(frame, &name));
                let value = match (value, fallback.first()) {
                    (Some(value), _) => Some(value),
                    (None, Some(fallback)) => Some(self.chunk(
                        css,
                        fallback.start,
                        fallback.end,
                        fallback.first_token_type,
                        fallback.last_token_type,
                        url_data,
                        frame,
                        references,
                    )?),
                    (None, None) => None,
                };
                value.map(|value| if tainted { value.attr_tainted() } else { value })
            },
            SubstitutionFunction::Ident(arguments) => {
                let [argument] = &**arguments else {
                    return Err(());
                };
                let argument = self.chunk(
                    css,
                    argument.start,
                    argument.end,
                    argument.first_token_type,
                    argument.last_token_type,
                    url_data,
                    frame,
                    references,
                )?;
                let context = crate::parser::ParserContext::new(
                    crate::stylesheets::Origin::Author,
                    url_data,
                    Some(crate::stylesheets::CssRuleType::Style),
                    style_traits::ParsingMode::DEFAULT,
                    self.computed_context.quirks_mode,
                    Default::default(),
                    None,
                    None,
                );
                let mut input = ParserInput::new(&argument.css);
                let mut input = Parser::new(&mut input);
                let property = self.early_random_property();
                let offset = self.reserve_early_random(&property, frame, 0);
                let context = context.for_random_declaration_with_offset(&property, offset, &mut input);
                self.reserve_early_random(&property, frame, context.random_function_count());
                let mut ident = String::new();
                while !input.is_exhausted() {
                    if let Ok(value) =
                        input.try_parse(|input| input.expect_ident_or_string().cloned())
                    {
                        ident.push_str(&value);
                    } else {
                        let number = crate::values::specified::Integer::parse(&context, &mut input)
                            .map_err(|_| ())?;
                        let scope = self.calls.random_scope(frame).unwrap_or_default().to_vec();
                        let previous = self.computed_context.replace_random_function_scope(scope);
                        let number = number.to_computed_value(self.computed_context);
                        let _ = self.computed_context.replace_random_function_scope(previous);
                        ident.push_str(&number.to_string());
                    }
                }
                if ident.is_empty() {
                    return Err(());
                }
                let mut css = String::new();
                cssparser::serialize_identifier(&ident, &mut css).map_err(|_| ())?;
                let value = Substitution::new(
                    Cow::Owned(css),
                    TokenSerializationType::Ident,
                    TokenSerializationType::Ident,
                );
                Some(if argument.attr_tainted.is_empty() {
                    value
                } else {
                    value.attr_tainted()
                })
            },
            SubstitutionFunction::RandomItem { arguments, identity, .. } => {
                let (index, tainted) = self.random_item_option(css, url_data, arguments, identity.as_ref(), reference.start, frame, references)?;
                let options = &arguments[1..];
                let chosen = &options[index];
                while references.next_if(|reference| reference.end <= chosen.start).is_some() {}
                let value = self.chunk(css, chosen.start, chosen.end, chosen.first_token_type, chosen.last_token_type, url_data, frame, references)?;
                Some(if tainted { value.attr_tainted() } else { value })
            },
            SubstitutionFunction::If(branches) => {
                Some(self.conditional(css, url_data, branches, reference.start, frame, references)?)
            },
            SubstitutionFunction::Inherit(arguments) => {
                let [name, fallback @ ..] = &**arguments else {
                    return Err(());
                };
                if fallback.len() > 1 {
                    return Err(());
                }
                let name = self
                    .chunk(
                        css,
                        name.start,
                        name.end,
                        name.first_token_type,
                        name.last_token_type,
                        url_data,
                        frame,
                        references,
                    )
                    .ok()
                    .and_then(|name| {
                        let mut input = ParserInput::new(&name.css);
                        Parser::new(&mut input)
                            .parse_entirely(|input| {
                                let name = input.expect_ident()?;
                                parse_name(name)
                                    .map(Atom::from)
                                    .map_err(|()| input.new_error_for_next_token::<()>())
                            })
                            .ok()
                    });
                let value = name.and_then(|name| self.inherit_function(frame, &name));
                match (value, fallback.first()) {
                    (Some(value), _) => Some(value),
                    (None, Some(fallback)) => Some(self.chunk(
                        css,
                        fallback.start,
                        fallback.end,
                        fallback.first_token_type,
                        fallback.last_token_type,
                        url_data,
                        frame,
                        references,
                    )?),
                    (None, None) => None,
                }
            },
            SubstitutionFunction::Env(name) => {
                match self.environment_resolution {
                    EnvironmentResolutionMode::ResolveLiveEnvironment => {
                        let device = self.stylist.device();
                        device
                            .environment()
                            .get(name, device, url_data)
                            .map(Substitution::from_value)
                    },
                    EnvironmentResolutionMode::TreatAsMissing => None,
                    EnvironmentResolutionMode::ResolvePagedMediaEnvOnly => {
                        // CSS env-1 §3 paged-media path: only the six
                        // paged-media variable names resolve; every other
                        // env() (notably safe-area-inset-*) falls through
                        // to its authored fallback, matching the previous
                        // `TreatAsMissing` behaviour for those names.
                        if is_paged_media_env_name(name) {
                            let device = self.stylist.device();
                            device
                                .environment()
                                .get(name, device, url_data)
                                .map(Substitution::from_value)
                        } else {
                            None
                        }
                    },
                }
            },
            SubstitutionFunction::Function { name, arguments, call_identity } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        self.chunk(
                            css,
                            argument.start,
                            argument.end,
                            argument.first_token_type,
                            argument.last_token_type,
                            url_data,
                            frame,
                            references,
                        )
                        .ok()
                        .map(Substitution::into_owned)
                    })
                    .collect();
                self.call(frame, name, arguments, url_data, call_identity.as_ref())
            },
            // https://drafts.csswg.org/css-values-5/#attr-substitution
            SubstitutionFunction::Attr {
                arguments,
                namespaces,
            } => {
                let [argument, fallback @ ..] = &**arguments else {
                    return Err(());
                };
                if fallback.len() > 1 {
                    return Err(());
                }
                let parsed = self
                    .chunk(
                        css,
                        argument.start,
                        argument.end,
                        argument.first_token_type,
                        argument.last_token_type,
                        url_data,
                        frame,
                        references,
                    )
                    .ok()
                    .and_then(|argument| {
                        let mut input = ParserInput::new(&argument.css);
                        let namespaces = Namespaces {
                            default: None,
                            prefixes: namespaces.iter().cloned().collect(),
                        };
                        Parser::new(&mut input)
                            .parse_entirely(|input| {
                                let name = AttrName::parse_with_namespaces(input, &namespaces)?;
                                Ok::<_, ParseError<'_>>((name, parse_attr_type(input)?))
                            })
                            .ok()
                    });
                let value = if let Some((name, syntax)) = parsed {
                    let entry = match syntax {
                        AttributeType::Type(_) => Some(self.calls.guard_attribute(&name)?),
                        _ => None,
                    };
                    let level = self.level_for(frame);
                    if level.shadow_scope_steps() > 0 {
                        self.computed_context.builder.add_flags(crate::computed_value_flags::ComputedValueFlags::DEPENDS_ON_ELEMENT_CONTEXT);
                        self.computed_context.rule_cache_conditions.borrow_mut().set_uncacheable();
                    }
                    let attribute = name.expanded_name().as_ref()
                        .and_then(|name| self.attribute_tracker.query_in_scope(name, level));
                    let value = attribute.map_or_else(
                        || {
                            // Special case when fallback and <attr-type> are omitted.
                            // See FAILURE: https://drafts.csswg.org/css-values-5/#attr-substitution
                            if fallback.is_empty() && syntax == AttributeType::None {
                                simple_subst("")
                            } else {
                                None
                            }
                        },
                        |attr| {
                            let mut input = ParserInput::new(&attr);
                            let mut parser = Parser::new(&mut input);
                            match &syntax {
                                AttributeType::Unit(unit) => {
                                    let css = {
                                        // Verify that attribute data is a <number-token>.
                                        parser
                                            .parse_entirely(|input| {
                                                Ok::<_, ParseError<'_>>(input.expect_number()?)
                                            })
                                            .ok()?;
                                        let mut s = attr.trim_ascii().to_owned();
                                        s.push_str(unit.as_ref());
                                        s
                                    };
                                    let serialization = match unit {
                                        AttrUnit::Number => TokenSerializationType::Number,
                                        AttrUnit::Percentage => TokenSerializationType::Percentage,
                                        _ => TokenSerializationType::Dimension,
                                    };
                                    let value = ComputedValue::new(
                                        css,
                                        url_data,
                                        serialization,
                                        serialization,
                                    );
                                    Some(Substitution::from_value(value))
                                },
                                AttributeType::Type(syntax) => {
                                    let mut value = VariableValue::parse(
                                        &mut parser,
                                        url_data,
                                        &Namespaces::default(),
                                    )
                                    .ok()?;
                                    value.private_names = self.private_names.clone();
                                    let value = self.substitute_value(&value, frame).ok()?;
                                    let mut input = ParserInput::new(&value.css);
                                    let mut parser = Parser::new(&mut input);
                                    let value = SpecifiedRegisteredValue::parse(
                                        &mut parser,
                                        syntax,
                                        url_data,
                                        AllowComputationallyDependent::Yes,
                                    )
                                    .ok()?;
                                    Some(Substitution::from_value(value.to_variable_value()))
                                },
                                AttributeType::RawString | AttributeType::None => {
                                    simple_subst(&attr)
                                },
                            }
                            .map(Substitution::attr_tainted)
                        },
                    );
                    if entry.is_some_and(|entry| self.calls.release(entry)) {
                        None
                    } else {
                        value
                    }
                } else {
                    None
                };
                match (value, fallback.first()) {
                    (Some(value), _) => Some(value),
                    (None, Some(fallback)) => Some(self.chunk(
                        css,
                        fallback.start,
                        fallback.end,
                        fallback.first_token_type,
                        fallback.last_token_type,
                        url_data,
                        frame,
                        references,
                    )?),
                    (None, None) => None,
                }
            },
        };

        if let Some(s) = substitution {
            // Skip references that are inside the outer variable (in fallback for example).
            while references
                .next_if(|next_ref| next_ref.end <= reference.end)
                .is_some()
            {}
            return Ok(if s.attr_tainted.is_empty() {
                s
            } else {
                s.attr_tainted()
            });
        }

        let Some(ref fallback) = reference.fallback else {
            return Err(());
        };

        // The closing parenthesis is outside the fallback token stream.
        let start = fallback.start.get();
        let end = reference.end - 1;
        self.chunk(
            css,
            start,
            end,
            fallback.first_token_type,
            fallback.last_token_type,
            url_data,
            frame,
            references,
        )
    }
}

fn quoted_css_string(src: &str) -> String {
    let mut dest = String::with_capacity(src.len() + 2);
    cssparser::serialize_string(src, &mut dest).unwrap();
    dest
}

/// The CSS text of a substituted value.
pub struct SubstitutedValue<'a> {
    /// The text.
    pub css: Cow<'a, str>,
    /// The ranges of `css` that `attr()` substitution produced.
    pub attr_tainted: ThinVec<AttrTaintedRange>,
    first_token_type: TokenSerializationType,
    last_token_type: TokenSerializationType,
}

impl SubstitutedValue<'_> {
    /// The substituted value, without references.
    pub fn into_value(self, url_data: &UrlExtraData) -> VariableValue {
        let mut value = VariableValue::new(
            self.css.into_owned(),
            url_data,
            self.first_token_type,
            self.last_token_type,
        );
        value.attr_tainted = self.attr_tainted;
        value
    }
}

/// Replace var(), env(), attr() and custom function calls, returning the resulting CSS string.
///
/// `level` is the cascade level of the declaration, which scopes its custom function names.
pub fn substitute<'a>(
    variable_value: &'a VariableValue,
    custom_properties: &'a ComputedCustomProperties,
    stylist: &'a Stylist,
    computed_context: &'a computed::Context,
    level: CascadeLevel,
    attribute_tracker: &'a mut AttributeTracker,
) -> Result<SubstitutedValue<'a>, ()> {
    substitute_with_environment_resolution(
        variable_value,
        custom_properties,
        stylist,
        computed_context,
        level,
        EnvironmentResolutionMode::ResolveLiveEnvironment,
        attribute_tracker,
    )
}

/// Replace var(), env(), attr() and custom function calls, returning the resulting CSS string.
pub fn substitute_with_environment_resolution<'a>(
    variable_value: &'a VariableValue,
    custom_properties: &'a ComputedCustomProperties,
    stylist: &'a Stylist,
    computed_context: &'a computed::Context,
    level: CascadeLevel,
    environment_resolution: EnvironmentResolutionMode,
    attribute_tracker: &'a mut AttributeTracker,
) -> Result<SubstitutedValue<'a>, ()> {
    debug_assert!(variable_value.has_references());
    Substitutor::new(
        custom_properties,
        stylist,
        computed_context,
        environment_resolution,
        attribute_tracker,
        Some(level),
    )
    .substitute_value(variable_value, None)
    .map(|substitution| SubstitutedValue {
        css: substitution.css,
        attr_tainted: substitution.attr_tainted,
        first_token_type: substitution.first_token_type,
        last_token_type: substitution.last_token_type,
    })
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::stylesheets::UrlExtraData;
    use crate::test_support::{BoolPrefGuard, pref_lock};
    use crate::{Namespace, Prefix};
    use cssparser::{Parser, ParserInput};
    use url::Url;

    #[test]
    fn private_function_arguments_retain_names_when_reparsed_as_dynamic_var() {
        let _guard = pref_lock().lock().unwrap();
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        let private = Atom::from("\0mixin-parameter");
        let argument = Arc::new(VariableValue::private_variable_reference(
            private.clone(),
            &url_data,
        ));
        let call = VariableValue::private_function_call(
            Atom::from("\0mixin-local"),
            &[argument],
            &url_data,
        )
        .unwrap();
        assert_eq!(
            call.references.refs[1].function.variable_name(),
            Some(&private)
        );
    }

    fn parse_value(css: &str, namespaces: &Namespaces) -> VariableValue {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| VariableValue::parse(input, &url_data, namespaces))
            .unwrap()
    }

    #[test]
    fn registered_colour_dependencies_track_accent_and_current_colour_tokens() {
        for (css, expected) in [
            ("AccentColor", NonCustomReferences::ACCENT_COLOR),
            (
                "color-mix(in srgb, AccentColorText, currentcolor)",
                NonCustomReferences::ACCENT_COLOR | NonCustomReferences::COLOR,
            ),
            ("red", NonCustomReferences::empty()),
            ("\"AccentColor\"", NonCustomReferences::empty()),
        ] {
            let value = parse_value(css, &Namespaces::default());
            let references = value.references.non_custom_references;
            assert_eq!(references, expected, "{css}");
            assert_eq!(
                non_custom_dependencies(DependentDataTypes::COLOR, false, references, false, false),
                (!expected.is_empty()).then_some(expected)
            );
        }
        assert_eq!(
            non_custom_dependencies(DependentDataTypes::COLOR, false, NonCustomReferences::empty(), true, false),
            Some(NonCustomReferences::empty())
        );
    }

    #[test]
    fn typed_function_dependencies_merge_font_and_colour_references() {
        use crate::shared_lock::StylesheetGuards;
        use crate::stylesheets::DocumentStyleSheet;
        use crate::test_support::{parse_stylesheet, test_device};
        let value = parse_value("--mixed()", &Namespaces::default());
        let _guard = pref_lock().lock().unwrap();
        let sheet = Arc::new(parse_stylesheet(
            "@function --mixed(--length <length>:1em, --color <color>:AccentColor) returns <color> \
             {result:color-mix(in srgb,var(--color),currentColor)}",
        ));
        let mut stylist = Stylist::new(test_device(), crate::context::QuirksMode::NoQuirks);
        let guard = sheet.shared_lock.read();
        stylist.append_stylesheet(DocumentStyleSheet(sheet.clone(), None), &guard);
        stylist.flush(&StylesheetGuards::same(&guard));
        let scope = functions::document_function_scope(&stylist, CascadeLevel::same_tree_author_normal());
        let dependencies = functions::FunctionDependencies::new(&value, &scope);
        let expected = NonCustomReferences::FONT_UNITS | NonCustomReferences::COLOR | NonCustomReferences::ACCENT_COLOR;
        assert_eq!(dependencies.non_custom_references, expected);
        for color_scheme in [false, true] {
            assert_eq!(function_non_custom_references(
                &dependencies, PropertyRegistrationData::unregistered(), &value, color_scheme, false, true,
            ), Some(expected));
        }
    }

    fn parsed_attr(value: &VariableValue) -> (AttrName, AttributeType) {
        match &value.references.refs[0].function {
            SubstitutionFunction::Attr {
                arguments,
                namespaces,
            } => {
                let argument = &arguments[0];
                let namespaces = Namespaces {
                    default: None,
                    prefixes: namespaces.iter().cloned().collect(),
                };
                let mut input = ParserInput::new(&value.css[argument.start..argument.end]);
                Parser::new(&mut input)
                    .parse_entirely(|input| {
                        Ok::<_, ParseError<'_>>((
                            AttrName::parse_with_namespaces(input, &namespaces)?,
                            parse_attr_type(input)?,
                        ))
                    })
                    .unwrap()
            },
            _ => panic!("expected an attr() reference"),
        }
    }

    #[test]
    fn native_ident_attribute_type_matches_the_standard_custom_ident_syntax() {
        let (_, legacy) = parsed_attr(&parse_value(
            "attr(commentstyle -bd-ident)",
            &Namespaces::default(),
        ));
        let (_, standard) = parsed_attr(&parse_value(
            "attr(commentstyle type(<custom-ident>))",
            &Namespaces::default(),
        ));
        let (AttributeType::Type(legacy), AttributeType::Type(standard)) = (legacy, standard)
        else {
            panic!("identifier attributes must use a syntax descriptor");
        };
        assert_eq!(legacy, standard);
    }

    #[test]
    fn consumed_eof_escapes_serialize_as_literal_replacement_characters() {
        for (authored, expected) in [
            (r"foo\", "foo�"),
            (r"1foo\", "1foo�"),
            (r"url(foo\", "url(foo�)"),
            (r#""foo\"#, "\"foo\""),
            ("foo�", "foo�"),
            (r"foo\fffd ", r"foo\fffd "),
            (r"foo\\", r"foo\\"),
        ] {
            let value: ComputedValue = parse_value(authored, &Namespaces::default());
            assert_eq!(value.to_css_string(), expected, "{authored}");
        }
        let value = parse_value(r"var(--missing, foo\", &Namespaces::default());
        assert_eq!(value.to_css_string(), "var(--missing, foo�)");
        let reference = &value.references.refs[0];
        assert_eq!(reference.end, value.css.len());
        let fallback = match &reference.function {
            SubstitutionFunction::DynamicVar { arguments, .. } => {
                let argument = arguments.last().unwrap();
                &value.css[argument.start..argument.end]
            },
            SubstitutionFunction::Var(_) => {
                &value.css[reference.fallback.as_ref().unwrap().start.get()..reference.end - 1]
            },
            _ => panic!("the fixture must retain a var() reference"),
        };
        assert_eq!(fallback, "foo�");
    }

    #[test]
    fn boundary_whitespace_trimming_keeps_escaped_identifier_characters() {
        assert_eq!(trim_token_whitespace(" \tkeep\\  \n"), r"keep\ ");
        assert_eq!(trim_token_whitespace(" fn(keep ) \n"), "fn(keep )");
        assert_eq!(trim_token_whitespace(" fn(keep \n"), "fn(keep \n");
        assert_eq!(trim_token_whitespace(" \"keep \" \n"), "\"keep \"");
        assert_eq!(trim_token_whitespace(" /*keep */ \n"), "/*keep */");
        assert_eq!(parse_value(r"keep\ ", &Namespaces::default()).css, r"keep\ ");
        assert!(!crate::properties_and_values::value::universal_values_match(Some(r"keep\ "), Some(r"keep\")));
    }

    #[test]
    fn nested_substitution_arguments_preserve_trailing_whitespace() {
        for css in [
            "if(else: keep )",
            "var(--missing, keep )",
            "attr(missing type(<length>), keep )",
            "--identity(keep )",
            "--identity({keep })",
        ] {
            let value = parse_value(&format!("{css}   "), &Namespaces::default());
            assert_eq!(value.css, css, "only the declaration boundary trims whitespace");
            let argument = match &value.references.refs[0].function {
                SubstitutionFunction::If(branches) => &branches[0].value,
                SubstitutionFunction::DynamicVar { arguments, .. }
                | SubstitutionFunction::Attr { arguments, .. }
                | SubstitutionFunction::Function { arguments, .. } => arguments.last().unwrap(),
                _ => panic!("the fixture has a nested substitution argument"),
            };
            assert_eq!(&value.css[argument.start..argument.end], "keep ", "{css}");
            assert_eq!(argument.last_token_type, token_types("keep ").1);
        }
        let value = parse_value("env(unknown-test-variable, keep )   ", &Namespaces::default());
        assert_eq!(value.css, "env(unknown-test-variable, keep )");
        let reference = &value.references.refs[0];
        let fallback = reference.fallback.as_ref().unwrap();
        assert_eq!(&value.css[fallback.start.get()..reference.end - 1], "keep ");
        assert_eq!(parse_value("keep   ", &Namespaces::default()).css, "keep");
    }

    #[test]
    fn conditional_arguments_require_a_branch_and_a_condition() {
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        for css in ["if()", "if( )", "if(: green)", "if(else: green; : red)"] {
            let mut input = ParserInput::new(css);
            assert!(
                Parser::new(&mut input)
                    .parse_entirely(|input| {
                        VariableValue::parse(input, &url_data, &Namespaces::default())
                    })
                    .is_err(),
                "{css}"
            );
        }
        for css in [
            "if(else:)",
            "if(else: green;)",
            "if(style(--x: 1): green; else: red)",
        ] {
            parse_value(css, &Namespaces::default());
        }
    }

    #[test]
    fn attr_references_store_expanded_names() {
        let mut namespaces = Namespaces::default();
        namespaces.prefixes.insert(
            Prefix::from("value"),
            Namespace::from("https://example.invalid/value"),
        );

        let value = parse_value("attr(value|colour type(*), red)", &namespaces);
        let (name, syntax) = parsed_attr(&value);
        assert_eq!(syntax, AttributeType::Type(Descriptor::universal()));
        assert_eq!(
            name,
            AttrName::Resolved {
                prefix: Some(Prefix::from("value")),
                namespace: Namespace::from("https://example.invalid/value"),
                local_name: Atom::from("colour"),
            }
        );
    }

    #[test]
    fn attr_namespace_prefixes_are_case_sensitive() {
        let mut namespaces = Namespaces::default();
        namespaces.prefixes.insert(
            Prefix::from("low"),
            Namespace::from("https://example.invalid/lower"),
        );
        namespaces.prefixes.insert(
            Prefix::from("LOW"),
            Namespace::from("https://example.invalid/upper"),
        );

        let lower = parse_value("attr(low|value)", &namespaces);
        let upper = parse_value("attr(LOW|value)", &namespaces);
        assert_ne!(parsed_attr(&lower).0, parsed_attr(&upper).0);
    }

    #[test]
    fn attr_local_names_preserve_authored_case() {
        let value = parse_value("attr(myAttr type(*), red)", &Namespaces::default());
        assert_eq!(parsed_attr(&value).0.local_name().as_ref(), "myAttr");
    }

    #[test]
    fn unbound_attr_namespace_is_a_typed_missing_attribute() {
        let value = parse_value(
            "attr(unbound|colour type(*), green)",
            &Namespaces::default(),
        );
        assert_eq!(
            parsed_attr(&value).0,
            AttrName::UnresolvedNamespace {
                prefix: Prefix::from("unbound"),
                local_name: Atom::from("colour"),
            }
        );
        assert!(matches!(&value.references.refs[0].function,
            SubstitutionFunction::Attr { arguments, .. } if arguments.len() == 2));
    }

    #[test]
    fn untyped_attr_fallback_remains_distinct_from_the_attribute_name() {
        let value = parse_value("attr(missing, \"fallback\")", &Namespaces::default());
        let (_, syntax) = parsed_attr(&value);
        assert_eq!(syntax, AttributeType::None);
        assert!(matches!(&value.references.refs[0].function,
            SubstitutionFunction::Attr { arguments, .. } if arguments.len() == 2));
    }

    #[test]
    fn random_item_argument_grammar_is_independent_of_key_and_chosen_value_grammar() {
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        for css in ["random-item(auto, serif)", "random-item(fixed -0.5, serif)", "random-item(var(--key), ,)", "random-item(auto, {}, {Times, serif})", "random-item(auto, --custom: value)"] {
            let mut input = ParserInput::new(css);
            assert!(Parser::new(&mut input).parse_entirely(|input| VariableValue::parse(input, &url_data, &Namespaces::default())).is_ok(), "{css}");
        }
        for css in ["random-item()", "random-item(auto)", "random-item(, serif)", "random-item(auto, !)", "random-item(auto, ;)", "random-item(auto, {Times, serif} extra)", "random-item(auto, extra {serif})"] {
            let mut input = ParserInput::new(css);
            assert!(Parser::new(&mut input).parse_entirely(|input| VariableValue::parse(input, &url_data, &Namespaces::default())).is_err(), "{css}");
        }
    }

}
