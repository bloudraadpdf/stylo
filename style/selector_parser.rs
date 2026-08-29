/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The pseudo-classes and pseudo-elements supported by the style system.

#![deny(missing_docs)]

use crate::derives::*;
use crate::stylesheets::{Namespaces, Origin, UrlExtraData};
use crate::values::{serialize_atom_identifier, AtomIdent};
use crate::Atom;
use cssparser::{match_ignore_ascii_case, Parser as CssParser, ParserInput};
use dom::ElementState;
use selectors::parser::{ParseRelative, SelectorList};
use std::fmt::{self, Debug, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// A convenient alias for the type that represents an attribute value used for
/// selector parser implementation.
pub type AttrValue = <SelectorImpl as ::selectors::SelectorImpl>::AttrValue;

#[cfg(feature = "servo")]
pub use crate::servo::selector_parser::*;

#[cfg(feature = "gecko")]
pub use crate::gecko::selector_parser::*;

#[cfg(feature = "servo")]
pub use crate::servo::selector_parser::ServoElementSnapshot as Snapshot;

#[cfg(feature = "gecko")]
pub use crate::gecko::snapshot::GeckoElementSnapshot as Snapshot;

#[cfg(feature = "servo")]
pub use crate::servo::restyle_damage::ServoRestyleDamage as RestyleDamage;

#[cfg(feature = "gecko")]
pub use crate::gecko::restyle_damage::GeckoRestyleDamage as RestyleDamage;

/// Servo's selector parser.
#[cfg_attr(feature = "servo", derive(MallocSizeOf))]
pub struct SelectorParser<'a> {
    /// The origin of the stylesheet we're parsing.
    pub stylesheet_origin: Origin,
    /// The namespace set of the stylesheet.
    pub namespaces: &'a Namespaces,
    /// The extra URL data of the stylesheet, which is used to look up
    /// whether we are parsing a chrome:// URL style sheet.
    pub url_data: &'a UrlExtraData,
    /// Whether we're parsing selectors for `@supports`
    pub for_supports_rule: bool,
}

impl<'a> SelectorParser<'a> {
    /// Parse a selector list with an author origin and without taking into
    /// account namespaces.
    ///
    /// This is used for some DOM APIs like `querySelector`.
    pub fn parse_author_origin_no_namespace<'i>(
        input: &'i str,
        url_data: &UrlExtraData,
    ) -> Result<SelectorList<SelectorImpl>, ParseError<'i>> {
        let namespaces = Namespaces::default();
        let parser = SelectorParser {
            stylesheet_origin: Origin::Author,
            namespaces: &namespaces,
            url_data,
            for_supports_rule: false,
        };
        let mut input = ParserInput::new(input);
        SelectorList::parse(&parser, &mut CssParser::new(&mut input), ParseRelative::No)
    }

    /// Whether we're parsing selectors in a user-agent stylesheet.
    pub fn in_user_agent_stylesheet(&self) -> bool {
        matches!(self.stylesheet_origin, Origin::UserAgent)
    }

    /// Whether we're parsing selectors in a stylesheet that has chrome
    /// privilege.
    pub fn chrome_rules_enabled(&self) -> bool {
        self.url_data.chrome_rules_enabled() || self.stylesheet_origin == Origin::User
    }
}

pub(crate) fn parse_lang_ranges<'i, 't>(
    input: &mut CssParser<'i, 't>,
) -> Result<Vec<AtomIdent>, ParseError<'i>> {
    let ranges = input.parse_comma_separated(|input| {
        Ok(AtomIdent::from(input.expect_ident_or_string()?.as_ref()))
    })?;
    if ranges.is_empty() {
        return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
    }
    Ok(ranges)
}

/// This enumeration determines how a pseudo-element cascades.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PseudoElementCascadeType {
    /// Eagerly cascaded pseudo-elements are "normal" pseudo-elements (i.e.
    /// `::before` and `::after`). They inherit styles normally as another
    /// selector would do, and they're computed as part of the cascade.
    ///
    /// These kind of pseudo-elements require more up-front computation and
    /// storage and thus should used for public pseudo-elements that can be used
    /// on many element types (such as `::before` and `::after`).
    Eager,
    /// Lazy pseudo-elements are affected by selector matching, but they're only
    /// computed when needed, and not before. They're useful for general
    /// pseudo-elements that are not very common or that do not apply to many
    /// elements. For instance in Servo this is used for `::backdrop` and
    /// `::marker`.
    Lazy,
    /// Precomputed pseudo-elements skip the cascade process entirely, mostly as
    /// an optimisation since they are private pseudo-elements (like
    /// `::-servo-details-content`).
    ///
    /// This pseudo-elements are resolved on the fly using *only* global rules
    /// (rules of the form `*|*`), and applying them to the parent style so are
    /// mainly useful for user-agent stylesheets.
    Precomputed,
}

/// A per-pseudo map, from a given pseudo to a `T`.
#[derive(Clone, MallocSizeOf)]
pub struct PerPseudoElementMap<T> {
    sparse: [i8; PSEUDO_COUNT],
    dense: Vec<T>,
}

impl<T> Default for PerPseudoElementMap<T> {
    fn default() -> Self {
        Self {
            dense: Vec::new(),
            sparse: [const { -1 }; PSEUDO_COUNT],
        }
    }
}

impl<T> Debug for PerPseudoElementMap<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('[')?;
        for idx in 0..=PSEUDO_COUNT {
            if idx > 0 {
                f.write_str(", ")?;
            }
            debug_assert!(self.sparse.get(idx).is_some());
            let i = self.sparse[idx];
            if i < 0 {
                None::<T>.fmt(f)?;
            } else {
                Some(&self.dense[i as usize]).fmt(f)?;
            }
        }
        f.write_char(']')
    }
}

impl<T> PerPseudoElementMap<T> {
    /// Get an entry in the map.
    pub fn get(&self, pseudo: &PseudoElement) -> Option<&T> {
        let idx = pseudo.index();
        debug_assert!(self.sparse.get(idx).is_some());
        let i = self.sparse[idx];
        if i < 0 {
            None
        } else {
            Some(&self.dense[i as usize])
        }
    }

    /// Clear this enumerated array.
    pub fn clear(&mut self) {
        self.dense.clear();
        self.sparse.fill(-1);
    }

    /// Set an entry value.
    ///
    /// Returns an error if the element is not a simple pseudo.
    pub fn set(&mut self, pseudo: &PseudoElement, value: T) {
        let idx = pseudo.index();
        let i = self.sparse[idx];
        if i < 0 {
            let i = self.dense.len() as i8;
            self.dense.push(value);
            self.sparse[idx] = i
        } else {
            self.dense[i as usize] = value
        }
    }

    /// Get an entry for `pseudo`, or create it with calling `f`.
    pub fn get_or_insert_with<F>(&mut self, pseudo: &PseudoElement, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        let idx = pseudo.index();
        let mut i = self.sparse[idx];
        if i < 0 {
            i = self.dense.len() as i8;
            debug_assert!(self.dense.len() < PSEUDO_COUNT);
            self.dense.push(f());
            self.sparse[idx] = i;
        }
        debug_assert!(self.dense.get(i as usize).is_some());
        &mut self.dense[i as usize]
    }

    /// Get an iterator for the entries.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.dense.iter()
    }

    /// Get a mutable iterator for the entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.dense.iter_mut()
    }
}

/// Values for the :dir() pseudo class
///
/// "ltr" and "rtl" values are normalized to lowercase.
#[derive(Clone, Debug, Eq, Hash, MallocSizeOf, PartialEq, ToShmem)]
pub struct Direction(pub Atom);

/// Horizontal values for the :dir() pseudo class
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HorizontalDirection {
    /// :dir(ltr)
    Ltr,
    /// :dir(rtl)
    Rtl,
}

impl Direction {
    /// Parse a direction value.
    pub fn parse<'i, 't>(parser: &mut CssParser<'i, 't>) -> Result<Self, ParseError<'i>> {
        let ident = parser.expect_ident()?;
        Ok(Direction(match_ignore_ascii_case! { &ident,
            "rtl" => atom!("rtl"),
            "ltr" => atom!("ltr"),
            _ => Atom::from(ident.as_ref()),
        }))
    }

    /// Convert this Direction into a HorizontalDirection, if applicable
    pub fn as_horizontal_direction(&self) -> Option<HorizontalDirection> {
        if self.0 == atom!("ltr") {
            Some(HorizontalDirection::Ltr)
        } else if self.0 == atom!("rtl") {
            Some(HorizontalDirection::Rtl)
        } else {
            None
        }
    }

    /// Gets the element state relevant to this :dir() selector.
    pub fn element_state(&self) -> ElementState {
        match self.as_horizontal_direction() {
            Some(HorizontalDirection::Ltr) => ElementState::LTR,
            Some(HorizontalDirection::Rtl) => ElementState::RTL,
            None => ElementState::empty(),
        }
    }
}

impl ToCss for Direction {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

#[cfg(test)]
mod tests {
    use cssparser::ToCss;

    use super::*;

    #[test]
    fn scroll_marker_selection_pseudo_classes_round_trip() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        for selector in [
            ".item::scroll-marker:target-current",
            ".item::scroll-marker:target-before",
            ".item::scroll-marker:target-after",
        ] {
            let parsed = SelectorParser::parse_author_origin_no_namespace(selector, &url_data)
                .expect("scroll marker selection pseudo-class must parse");
            assert_eq!(parsed.to_css_string(), selector);
        }
    }

    #[test]
    fn level_five_heading_selectors_validate_and_round_trip() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        for selector in [":heading", ":heading(0, 1, -2)", "h1:heading(1)"] {
            let parsed = SelectorParser::parse_author_origin_no_namespace(selector, &url_data)
                .expect("heading selectors must parse at author origin");
            assert_eq!(parsed.to_css_string(), selector);
        }
        for selector in [":heading()", ":heading(1.0)", ":heading(2n)"] {
            assert!(
                SelectorParser::parse_author_origin_no_namespace(selector, &url_data).is_err(),
                "{selector} must be rejected"
            );
        }
    }

    #[test]
    fn public_part_descendants_validate_and_round_trip() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        for selector in [
            "::part(foo)::placeholder",
            "::part(foo)::file-selector-button",
            "::part(foo):lang(en)",
        ] {
            let parsed = SelectorParser::parse_author_origin_no_namespace(selector, &url_data)
                .expect("public selectors after ::part() must parse at author origin");
            assert_eq!(parsed.to_css_string(), selector);
        }
    }

    #[test]
    fn can_build_and_set_arbitrary_index() {
        let mut map = <PerPseudoElementMap<i32>>::default();
        assert_eq!(map.get(&PseudoElement::After), None);
        map.set(&PseudoElement::After, 3);
        assert_eq!(map.get(&PseudoElement::After), Some(3).as_ref());

        assert_eq!(map.get(&PseudoElement::Backdrop), None);
        map.set(&PseudoElement::Backdrop, 8);
        assert_eq!(map.get(&PseudoElement::Backdrop), Some(8).as_ref());

        assert_eq!(
            map.get_or_insert_with(&PseudoElement::Backdrop, || { 10 }),
            &8
        );
        map.set(&PseudoElement::Backdrop, 9);
        assert_eq!(map.get(&PseudoElement::Backdrop), Some(9).as_ref());

        assert_eq!(
            map.get_or_insert_with(&PseudoElement::Marker, || { 10 }),
            &10
        );
        assert_eq!(map.get(&PseudoElement::Marker), Some(10).as_ref());
    }

    #[test]
    fn can_iter() {
        let mut map = <PerPseudoElementMap<i32>>::default();
        map.set(&PseudoElement::After, 3);
        map.set(&PseudoElement::Backdrop, 8);
        assert_eq!(map.iter().cloned().collect::<Vec<_>>(), vec![3, 8]);
    }
}
