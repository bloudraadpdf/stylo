/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A [`@page`][page] rule.
//!
//! [page]: https://drafts.csswg.org/css2/page.html#page-box

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::properties::PropertyDeclarationBlock;
use crate::shared_lock::{
    DeepCloneWithLock, Locked, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard,
};
use crate::stylesheets::{style_or_page_rule_to_css, CssRules};
use crate::values::{AtomIdent, CustomIdent};
use cssparser::parse_nth;
use cssparser::{match_ignore_ascii_case, Parser, SourceLocation, Token};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use smallvec::SmallVec;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, ToCss};

macro_rules! page_pseudo_classes {
    ($($(#[$($meta:tt)+])* $id:ident => $val:literal,)+) => {
        /// [`@page`][page] rule pseudo-classes.
        ///
        /// https://drafts.csswg.org/css-page-3/#page-selectors
        #[derive(Clone, Copy, Debug, Eq, MallocSizeOf, PartialEq, ToShmem)]
        #[repr(u8)]
        pub enum PagePseudoClass {
            $($(#[$($meta)+])* $id,)+
        }
        impl PagePseudoClass {
            fn parse<'i, 't>(
                input: &mut Parser<'i, 't>,
            ) -> Result<Self, ParseError<'i>> {
                let loc = input.current_source_location();
                let colon = input.next_including_whitespace()?;
                if *colon != Token::Colon {
                    return Err(loc.new_unexpected_token_error(colon.clone()));
                }

                let ident = input.next_including_whitespace()?;
                if let Token::Ident(s) = ident {
                    return match_ignore_ascii_case! { &**s,
                        $($val => Ok(PagePseudoClass::$id),)+
                        _ => Err(loc.new_unexpected_token_error(Token::Ident(s.clone()))),
                    };
                }
                Err(loc.new_unexpected_token_error(ident.clone()))
            }
            #[inline]
            fn to_str(&self) -> &'static str {
                match *self {
                    $(PagePseudoClass::$id => concat!(':', $val),)+
                }
            }
        }
    }
}

page_pseudo_classes! {
    /// [`:first`][first] pseudo-class
    ///
    /// [first] https://drafts.csswg.org/css-page-3/#first-pseudo
    First => "first",
    /// [`:blank`][blank] pseudo-class
    ///
    /// [blank] https://drafts.csswg.org/css-page-3/#blank-pseudo
    Blank => "blank",
    /// [`:left`][left] pseudo-class
    ///
    /// [left]: https://drafts.csswg.org/css-page-3/#spread-pseudos
    Left => "left",
    /// [`:right`][right] pseudo-class
    ///
    /// [right]: https://drafts.csswg.org/css-page-3/#spread-pseudos
    Right => "right",
    /// [`:recto`][recto] pseudo-class
    ///
    /// Direction-aware spread pseudo: equivalent to `:right` in
    /// left-to-right page progression and `:left` in right-to-left.
    ///
    /// [recto]: https://drafts.csswg.org/css-page-3/#spread-pseudos
    Recto => "recto",
    /// [`:verso`][verso] pseudo-class
    ///
    /// Direction-aware spread pseudo: equivalent to `:left` in
    /// left-to-right page progression and `:right` in right-to-left.
    ///
    /// [verso]: https://drafts.csswg.org/css-page-3/#spread-pseudos
    Verso => "verso",
    /// moegoe Family 30 — `:first-of-group` page pseudo-class
    /// (Prince `prince.md:6567,8255`).
    ///
    /// Matches the first page in a page group as established by
    /// `-bd-page-group: start` (Prince `-prince-page-group`). The
    /// paginator sets the flag at every forced page-break-before that
    /// originates a new group. Specificity is grouped with `:first` /
    /// `:blank` (the `g` bucket) since this is also a positional
    /// page-context pseudo with no direction component.
    FirstOfGroup => "first-of-group",
    /// moegoe Family 23 — `:index` page pseudo-class.
    ///
    /// Matches pages synthesised by the GCPM book-index area
    /// renderer (one or more `@-bd-index` rule sets). The paginator
    /// sets the flag for every page emitted from the index back-
    /// matter pass; no other page surface engages it. Specificity
    /// is grouped with `:first` / `:blank` (the `g` bucket) since
    /// this is also a positional page-context pseudo with no
    /// direction component.
    Index => "index",
    /// moegoe — `:last` page pseudo-class (PDFreactor item 20 / C.6,
    /// `pdfreactor.md:4107,19780`).
    ///
    /// Matches the final page of the document. moegoe's paginator
    /// can only set this flag once the total page count is known,
    /// so the cascade-resolution path re-evaluates last-page
    /// settings as a finalisation pass post-pagination. Specificity
    /// is grouped with `:first` / `:blank` / `:first-of-group` /
    /// `:index` (the `g` bucket) since this is a positional
    /// page-context pseudo with no direction component.
    Last => "last",
}

bitflags! {
    /// Bit-flags for pseudo-class. This should only be used for querying if a
    /// page-rule applies.
    ///
    /// Widened to `u16` in moegoe Family 23 to accommodate `:index` once
    /// the eight `u8` bits were exhausted by the existing pseudos.
    ///
    /// https://drafts.csswg.org/css-page-3/#page-selectors
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct PagePseudoClassFlags : u16 {
        /// No pseudo-classes
        const NONE = 0;
        /// Flag for PagePseudoClass::First
        const FIRST = 1 << 0;
        /// Flag for PagePseudoClass::Blank
        const BLANK = 1 << 1;
        /// Flag for PagePseudoClass::Left
        const LEFT = 1 << 2;
        /// Flag for PagePseudoClass::Right
        const RIGHT = 1 << 3;
        /// Flag for `:nth(An+B)` presence
        const NTH = 1 << 4;
        /// Flag for PagePseudoClass::Recto
        const RECTO = 1 << 5;
        /// Flag for PagePseudoClass::Verso
        const VERSO = 1 << 6;
        /// Flag for PagePseudoClass::FirstOfGroup (Family 30)
        const FIRST_OF_GROUP = 1 << 7;
        /// Flag for PagePseudoClass::Index (Family 23 — book-index
        /// back-matter pages synthesised by the GCPM index renderer).
        const INDEX = 1 << 8;
        /// Flag for PagePseudoClass::Last (PDFreactor item 20 / C.6 —
        /// matches the final page of the document, set as a
        /// finalisation pass once total page count is known).
        const LAST = 1 << 9;
    }
}

impl PagePseudoClassFlags {
    /// Creates a pseudo-class flags object with a single pseudo-class.
    #[inline]
    pub fn new(other: &PagePseudoClass) -> Self {
        match *other {
            PagePseudoClass::First => PagePseudoClassFlags::FIRST,
            PagePseudoClass::Blank => PagePseudoClassFlags::BLANK,
            PagePseudoClass::Left => PagePseudoClassFlags::LEFT,
            PagePseudoClass::Right => PagePseudoClassFlags::RIGHT,
            PagePseudoClass::Recto => PagePseudoClassFlags::RECTO,
            PagePseudoClass::Verso => PagePseudoClassFlags::VERSO,
            PagePseudoClass::FirstOfGroup => PagePseudoClassFlags::FIRST_OF_GROUP,
            PagePseudoClass::Index => PagePseudoClassFlags::INDEX,
            PagePseudoClass::Last => PagePseudoClassFlags::LAST,
        }
    }
    /// Checks if the given pseudo class applies to this set of flags.
    #[inline]
    pub fn contains_class(self, other: &PagePseudoClass) -> bool {
        self.intersects(PagePseudoClassFlags::new(other))
    }
}

type PagePseudoClasses = SmallVec<[PagePseudoClass; 4]>;

/// The `:nth(An+B)` components of one page selector.
///
/// A page selector is a compound selector, so `@page:nth(n+3):nth(-n+4)`
/// carries TWO `:nth()` components and matches only the pages both accept.
/// One inline slot covers every real stylesheet; a chain of two appears in
/// PDFreactor's own `magazine` sample.
type PageNths = SmallVec<[(i32, i32); 1]>;

/// Type of a single [`@page`][page selector]
///
/// [page-selectors]: https://drafts.csswg.org/css2/page.html#page-selectors
#[derive(Clone, Debug, MallocSizeOf, ToShmem)]
pub struct PageSelector {
    /// Page name
    ///
    /// https://drafts.csswg.org/css-page-3/#page-type-selector
    pub name: AtomIdent,
    /// Pseudo-classes for [`@page`][page-selectors]
    ///
    /// [page-selectors]: https://drafts.csswg.org/css2/page.html#page-selectors
    pub pseudos: PagePseudoClasses,
    /// `:nth(An+B)` coefficients, one per `:nth()` component. Empty when the
    /// selector carries none.
    ///
    /// A compound page selector may repeat `:nth()`; every component must
    /// match, exactly as the keyword pseudo-classes in `pseudos` must.
    ///
    /// https://drafts.csswg.org/css-page-4/#nth-page-pseudo-class
    pub nth: PageNths,
}

/// Whether `:nth(An+B)` selects `page_number` (1-indexed).
///
/// https://drafts.csswg.org/css-page-4/#nth-page-pseudo-class
#[inline]
fn nth_matches(a: i32, b: i32, page_number: usize) -> bool {
    let n = page_number as i32;
    let diff = n - b;
    if a == 0 {
        diff == 0
    } else {
        diff % a == 0 && diff / a >= 0
    }
}

/// Computes the [specificity] given the g, h, and f values as in the spec.
///
/// g is number of `:first` or `:blank`, h is number of `:left` or `:right`,
/// f is if the selector includes a page-name (selectors can only include one
/// or zero page-names).
///
/// This places hard limits of 65535 on h and 32767 on g, at which point all
/// higher values are treated as those limits respectively.
///
/// [specificity]: https://drafts.csswg.org/css-page/#specificity
#[inline]
fn selector_specificity(g: usize, h: usize, f: bool) -> u32 {
    let h = h.min(0xFFFF) as u32;
    let g = (g.min(0x7FFF) as u32) << 16;
    let f = if f { 0x80000000 } else { 0 };
    h + g + f
}

impl PageSelector {
    /// Checks if the ident matches a page-name's ident.
    ///
    /// This does not take pseudo selectors into account.
    #[inline]
    pub fn ident_matches(&self, other: &CustomIdent) -> bool {
        self.name.0 == other.0
    }

    /// Checks that this selector matches the ident and all pseudo classes are
    /// present in the provided flags.
    #[inline]
    pub fn matches(
        &self,
        name: &CustomIdent,
        flags: PagePseudoClassFlags,
        page_number: usize,
    ) -> bool {
        self.ident_matches(name) && self.flags_match(flags, page_number)
    }

    /// Checks that all pseudo classes in this selector are present in the
    /// provided flags, and that `:nth()` matches the given page number.
    ///
    /// Equivalent to, but may be more efficient than:
    ///
    /// ```
    /// match_specificity(flags, page_number).is_some()
    /// ```
    pub fn flags_match(&self, flags: PagePseudoClassFlags, page_number: usize) -> bool {
        if !self.pseudos.iter().all(|pc| flags.contains_class(pc)) {
            return false;
        }
        if !self
            .nth
            .iter()
            .all(|&(a, b)| nth_matches(a, b, page_number))
        {
            return false;
        }
        true
    }

    /// Implements specificity calculation for a page selector given a set of
    /// page pseudo-classes to match with.
    /// If this selector includes any pseudo-classes that are not in the flags,
    /// then this will return None.
    ///
    /// To fit the specificity calculation into a 32-bit value, this limits the
    /// maximum count of :first and :blank to 32767, and the maximum count of
    /// :left and :right to 65535.
    ///
    /// https://drafts.csswg.org/css-page-3/#cascading-and-page-context
    pub fn match_specificity(
        &self,
        flags: PagePseudoClassFlags,
        page_number: usize,
    ) -> Option<u32> {
        let mut g: usize = 0;
        let mut h: usize = 0;
        for pc in self.pseudos.iter() {
            if !flags.contains_class(pc) {
                return None;
            }
            match pc {
                PagePseudoClass::First
                | PagePseudoClass::Blank
                | PagePseudoClass::FirstOfGroup
                | PagePseudoClass::Index
                | PagePseudoClass::Last => g += 1,
                PagePseudoClass::Left
                | PagePseudoClass::Right
                | PagePseudoClass::Recto
                | PagePseudoClass::Verso => h += 1,
            }
        }
        // Check every :nth() component. Each contributes +1 pseudo-class
        // specificity (same bucket as :left / :right).
        for &(a, b) in self.nth.iter() {
            if !nth_matches(a, b, page_number) {
                return None;
            }
            h += 1;
        }
        Some(selector_specificity(g, h, !self.name.0.is_empty()))
    }
}

impl ToCss for PageSelector {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.name.to_css(dest)?;
        for pc in self.pseudos.iter() {
            dest.write_str(pc.to_str())?;
        }
        for &(a, b) in self.nth.iter() {
            dest.write_str(":nth(")?;
            match (a, b) {
                (0, val) => write!(dest, "{}", val)?,
                (1, 0) => dest.write_str("n")?,
                (-1, 0) => dest.write_str("-n")?,
                (val, 0) => write!(dest, "{}n", val)?,
                (1, val) if val > 0 => write!(dest, "n+{}", val)?,
                (1, val) => write!(dest, "n{}", val)?,
                (-1, val) if val > 0 => write!(dest, "-n+{}", val)?,
                (-1, val) => write!(dest, "-n{}", val)?,
                (a_val, val) if val > 0 => write!(dest, "{}n+{}", a_val, val)?,
                (a_val, val) => write!(dest, "{}n{}", a_val, val)?,
            }
            dest.write_char(')')?;
        }
        Ok(())
    }
}

fn parse_page_name<'i, 't>(input: &mut Parser<'i, 't>) -> Result<AtomIdent, ParseError<'i>> {
    let s = input.expect_ident()?;
    Ok(AtomIdent::from(&**s))
}

impl Parse for PageSelector {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let name = input.try_parse(parse_page_name);
        let mut pseudos = PagePseudoClasses::default();
        let mut nth = PageNths::default();
        loop {
            // Try functional pseudo-class :nth(...) first.
            // We use a state variable since try_parse borrows input mutably.
            let parsed_nth = input.try_parse(|i| -> Result<(i32, i32), ParseError<'i>> {
                let loc = i.current_source_location();
                let colon = i.next_including_whitespace()?;
                if *colon != Token::Colon {
                    return Err(
                        loc.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
                i.expect_function_matching("nth")?;
                i.parse_nested_block(|i| parse_nth(i).map_err(|e| e.into()))
            });
            if let Ok((a, b)) = parsed_nth {
                nth.push((a, b));
                continue;
            }
            // Then try keyword pseudo-class
            if let Ok(pc) = input.try_parse(PagePseudoClass::parse) {
                pseudos.push(pc);
                continue;
            }
            break;
        }
        // If the result was empty, then we didn't get a selector.
        let has_content = !pseudos.is_empty() || !nth.is_empty();
        let name = match name {
            Ok(name) => name,
            Err(..) if has_content => AtomIdent::new(atom!("")),
            Err(err) => return Err(err),
        };
        Ok(PageSelector { name, pseudos, nth })
    }
}

/// A list of [`@page`][page selectors]
///
/// [page-selectors]: https://drafts.csswg.org/css2/page.html#page-selectors
#[derive(Clone, Debug, Default, MallocSizeOf, ToCss, ToShmem)]
#[css(comma)]
pub struct PageSelectors(#[css(iterable)] pub Box<[PageSelector]>);

impl PageSelectors {
    /// Creates a new PageSelectors from a Vec, as from parse_comma_separated
    #[inline]
    pub fn new(s: Vec<PageSelector>) -> Self {
        PageSelectors(s.into())
    }
    /// Returns true iff there are any page selectors
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
    /// Get the underlying PageSelector data as a slice
    #[inline]
    pub fn as_slice(&self) -> &[PageSelector] {
        &*self.0
    }
}

impl Parse for PageSelectors {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(PageSelectors::new(input.parse_comma_separated(|i| {
            PageSelector::parse(context, i)
        })?))
    }
}

/// A [`@page`][page] rule.
///
/// This implements only a limited subset of the CSS
/// 2.2 syntax.
///
/// [page]: https://drafts.csswg.org/css2/page.html#page-box
/// [page-selectors]: https://drafts.csswg.org/css2/page.html#page-selectors
#[derive(Clone, Debug, ToShmem)]
pub struct PageRule {
    /// Selectors of the page-rule
    pub selectors: PageSelectors,
    /// Nested rules.
    pub rules: Arc<Locked<CssRules>>,
    /// The declaration block this page rule contains.
    pub block: Arc<Locked<PropertyDeclarationBlock>>,
    /// The source position this rule was found at.
    pub source_location: SourceLocation,
}

impl PageRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        // Measurement of other fields may be added later.
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
            + self.block.unconditional_shallow_size_of(ops)
            + self.block.read_with(guard).size_of(ops)
            + self.selectors.size_of(ops)
    }
    /// Computes the specificity of this page rule when matched with flags.
    ///
    /// Computing this value has linear-complexity with the size of the
    /// selectors, so the caller should usually call this once and cache the
    /// result.
    ///
    /// Returns None if the flags do not match this page rule.
    ///
    /// The return type is ordered by page-rule specificity.
    pub fn match_specificity(
        &self,
        flags: PagePseudoClassFlags,
        page_number: usize,
    ) -> Option<u32> {
        if self.selectors.is_empty() {
            // A page-rule with no selectors matches all pages, but with the
            // lowest possible specificity.
            return Some(selector_specificity(0, 0, false));
        }
        let mut specificity = None;
        for s in self
            .selectors
            .0
            .iter()
            .map(|s| s.match_specificity(flags, page_number))
        {
            specificity = s.max(specificity);
        }
        specificity
    }
}

impl ToCssWithGuard for PageRule {
    /// Serialization of PageRule is not specced, adapted from steps for StyleRule.
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        // https://drafts.csswg.org/cssom/#serialize-a-css-rule
        dest.write_str("@page ")?;
        if !self.selectors.is_empty() {
            self.selectors.to_css(&mut CssWriter::new(dest))?;
            dest.write_char(' ')?;
        }
        style_or_page_rule_to_css(Some(&self.rules), &self.block, guard, dest)
    }
}

impl DeepCloneWithLock for PageRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(&guard);
        PageRule {
            selectors: self.selectors.clone(),
            block: Arc::new(lock.wrap(self.block.read_with(&guard).clone())),
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            source_location: self.source_location.clone(),
        }
    }
}
