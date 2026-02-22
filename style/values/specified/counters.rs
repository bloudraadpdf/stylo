/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified types for counter properties.

#[cfg(feature = "servo")]
use crate::computed_values::list_style_type::T as ListStyleType;
#[cfg(feature = "gecko")]
use crate::counter_style::CounterStyle;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::counters as generics;
use crate::values::generics::counters::CounterPair;
use crate::values::specified::image::Image;
use crate::values::specified::Attr;
use crate::values::specified::Integer;
use crate::values::CustomIdent;
use cssparser::{match_ignore_ascii_case, Parser, Token};
use selectors::parser::SelectorParseErrorKind;
use style_traits::{ParseError, StyleParseErrorKind};

#[derive(PartialEq)]
enum CounterType {
    Increment,
    Set,
    Reset,
}

impl CounterType {
    fn default_value(&self) -> i32 {
        match *self {
            Self::Increment => 1,
            Self::Reset | Self::Set => 0,
        }
    }
}

/// A specified value for the `counter-increment` property.
pub type CounterIncrement = generics::GenericCounterIncrement<Integer>;

impl Parse for CounterIncrement {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self::new(parse_counters(
            context,
            input,
            CounterType::Increment,
        )?))
    }
}

/// A specified value for the `counter-set` property.
pub type CounterSet = generics::GenericCounterSet<Integer>;

impl Parse for CounterSet {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self::new(parse_counters(context, input, CounterType::Set)?))
    }
}

/// A specified value for the `counter-reset` property.
pub type CounterReset = generics::GenericCounterReset<Integer>;

impl Parse for CounterReset {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self::new(parse_counters(
            context,
            input,
            CounterType::Reset,
        )?))
    }
}

fn parse_counters<'i, 't>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
    counter_type: CounterType,
) -> Result<Vec<CounterPair<Integer>>, ParseError<'i>> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(vec![]);
    }

    let mut counters = Vec::new();
    loop {
        let location = input.current_source_location();
        let (name, is_reversed) = match input.next() {
            Ok(&Token::Ident(ref ident)) => {
                (CustomIdent::from_ident(location, ident, &["none"])?, false)
            },
            Ok(&Token::Function(ref name))
                if counter_type == CounterType::Reset && name.eq_ignore_ascii_case("reversed") =>
            {
                input
                    .parse_nested_block(|input| Ok((CustomIdent::parse(input, &["none"])?, true)))?
            },
            Ok(t) => {
                let t = t.clone();
                return Err(location.new_unexpected_token_error(t));
            },
            Err(_) => break,
        };

        let value = match input.try_parse(|input| Integer::parse(context, input)) {
            Ok(start) => {
                if start.value() == i32::min_value() {
                    // The spec says that values must be clamped to the valid range,
                    // and we reserve i32::min_value() as an internal magic value.
                    // https://drafts.csswg.org/css-lists/#auto-numbering
                    Integer::new(i32::min_value() + 1)
                } else {
                    start
                }
            },
            _ => Integer::new(if is_reversed {
                i32::min_value()
            } else {
                counter_type.default_value()
            }),
        };
        counters.push(CounterPair {
            name,
            value,
            is_reversed,
        });
    }

    if !counters.is_empty() {
        Ok(counters)
    } else {
        Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}

/// The specified value for the `content` property.
pub type Content = generics::GenericContent<Image>;

/// The specified value for a content item in the `content` property.
pub type ContentItem = generics::GenericContentItem<Image>;

/// The specified value for the `string-set` property.
pub type StringSet = generics::GenericStringSet<Image>;

fn parse_string_lookup_keyword<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<generics::StringLookupKeyword, ParseError<'i>> {
    let ident = input.expect_ident()?;
    Ok(match_ignore_ascii_case! { ident,
        "first" => generics::StringLookupKeyword::First,
        "start" => generics::StringLookupKeyword::Start,
        "last" => generics::StringLookupKeyword::Last,
        "first-except" => generics::StringLookupKeyword::FirstExcept,
        _ => {
            let ident = ident.clone();
            return Err(input.new_custom_error(
                SelectorParseErrorKind::UnexpectedIdent(ident)
            ));
        }
    })
}

fn parse_content_function_keyword<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<generics::StringSetContentKeyword, ParseError<'i>> {
    if input.is_exhausted() {
        return Ok(generics::StringSetContentKeyword::Text);
    }

    let ident = input.expect_ident()?;
    let keyword = match_ignore_ascii_case! { ident,
        "text" => generics::StringSetContentKeyword::Text,
        "before" => generics::StringSetContentKeyword::Before,
        "after" => generics::StringSetContentKeyword::After,
        "first-letter" => generics::StringSetContentKeyword::FirstLetter,
        _ => {
            let ident = ident.clone();
            return Err(input.new_custom_error(
                SelectorParseErrorKind::UnexpectedIdent(ident)
            ));
        }
    };

    input.expect_exhausted()?;
    Ok(keyword)
}

fn parse_content_item<'i, 't>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
    allow_images: bool,
    allow_counter_functions: bool,
    allow_string_functions: bool,
    allow_element_functions: bool,
    allow_content_function: bool,
    allow_quote_idents: bool,
) -> Result<ContentItem, ParseError<'i>> {
    if allow_images {
        if let Ok(image) = input.try_parse(|i| Image::parse_forbid_none(context, i)) {
            return Ok(generics::ContentItem::Image(image));
        }
    }

    let token = input.next()?.clone();
    match token {
        Token::QuotedString(ref value) => Ok(generics::ContentItem::String(
            value.as_ref().to_owned().into(),
        )),
        Token::Function(ref name) => {
            match_ignore_ascii_case! { &name,
                "counter" if allow_counter_functions => input.parse_nested_block(|input| {
                    let name = CustomIdent::parse(input, &[])?;
                    let style = Content::parse_counter_style(context, input);
                    Ok(generics::ContentItem::Counter(name, style))
                }),
                "counters" if allow_counter_functions => input.parse_nested_block(|input| {
                    let name = CustomIdent::parse(input, &[])?;
                    input.expect_comma()?;
                    let separator = input.expect_string()?.as_ref().to_owned().into();
                    let style = Content::parse_counter_style(context, input);
                    Ok(generics::ContentItem::Counters(name, separator, style))
                }),
                "string" if allow_string_functions => input.parse_nested_block(|input| {
                    let name = CustomIdent::parse(input, &[])?;
                    let keyword = input
                        .try_parse(|input| {
                            input.expect_comma()?;
                            parse_string_lookup_keyword(input)
                        })
                        .unwrap_or(generics::StringLookupKeyword::First);
                    Ok(generics::ContentItem::StringFunction(name, keyword))
                }),
                "element" if allow_element_functions => input.parse_nested_block(|input| {
                    let name = CustomIdent::parse(input, &[])?;
                    let keyword = input
                        .try_parse(|input| {
                            input.expect_comma()?;
                            parse_string_lookup_keyword(input)
                        })
                        .unwrap_or(generics::StringLookupKeyword::First);
                    Ok(generics::ContentItem::ElementFunction(name, keyword))
                }),
                "content" if allow_content_function => input.parse_nested_block(|input| {
                    Ok(generics::ContentItem::ContentFunction(
                        parse_content_function_keyword(input)?,
                    ))
                }),
                "attr" if !static_prefs::pref!("layout.css.attr.enabled") => input.parse_nested_block(|input| {
                    Ok(generics::ContentItem::Attr(Attr::parse_function(context, input)?))
                }),
                _ => {
                    let name = name.clone();
                    Err(input.new_custom_error(StyleParseErrorKind::UnexpectedFunction(name)))
                }
            }
        },
        Token::Ident(ref ident) if allow_quote_idents => Ok(match_ignore_ascii_case! { &ident,
            "open-quote" => generics::ContentItem::OpenQuote,
            "close-quote" => generics::ContentItem::CloseQuote,
            "no-open-quote" => generics::ContentItem::NoOpenQuote,
            "no-close-quote" => generics::ContentItem::NoCloseQuote,
            #[cfg(feature = "gecko")]
            "-moz-alt-content" if context.in_ua_sheet() => {
                generics::ContentItem::MozAltContent
            },
            #[cfg(feature = "gecko")]
            "-moz-label-content" if context.chrome_rules_enabled() => {
                generics::ContentItem::MozLabelContent
            },
            _ =>{
                let ident = ident.clone();
                return Err(input.new_custom_error(
                    SelectorParseErrorKind::UnexpectedIdent(ident)
                ));
            }
        }),
        token => Err(input.new_unexpected_token_error(token)),
    }
}

impl Content {
    #[cfg(feature = "servo")]
    fn parse_counter_style(_: &ParserContext, input: &mut Parser) -> ListStyleType {
        input
            .try_parse(|input| {
                input.expect_comma()?;
                ListStyleType::parse(input)
            })
            .unwrap_or(ListStyleType::Decimal)
    }

    #[cfg(feature = "gecko")]
    fn parse_counter_style(context: &ParserContext, input: &mut Parser) -> CounterStyle {
        use crate::counter_style::CounterStyleParsingFlags;
        input
            .try_parse(|input| {
                input.expect_comma()?;
                CounterStyle::parse(context, input, CounterStyleParsingFlags::empty())
            })
            .unwrap_or_else(|_| CounterStyle::decimal())
    }
}

impl Parse for Content {
    // normal | none | [ <string> | <counter> | open-quote | close-quote | no-open-quote |
    // no-close-quote ]+
    // TODO: <uri>, attr(<identifier>)
    #[cfg_attr(feature = "servo", allow(unused_mut))]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(generics::Content::Normal);
        }
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(generics::Content::None);
        }

        let mut items = thin_vec::ThinVec::new();
        let mut alt_start = None;
        loop {
            if input.is_exhausted() {
                break;
            }
            if alt_start.is_none()
                && !items.is_empty()
                && static_prefs::pref!("layout.css.content.alt-text.enabled")
                && input.try_parse(|input| input.expect_delim('/')).is_ok()
            {
                alt_start = Some(items.len());
                continue;
            }

            let item = parse_content_item(
                context,
                input,
                alt_start.is_none(),
                alt_start.is_none(),
                alt_start.is_none(),
                alt_start.is_none(),
                false,
                alt_start.is_none(),
            )?;
            items.push(item);
        }
        if items.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let alt_start = alt_start.unwrap_or(items.len());
        Ok(generics::Content::Items(generics::GenericContentItems {
            items,
            alt_start,
        }))
    }
}

impl Parse for StringSet {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            input.expect_exhausted()?;
            return Ok(StringSet::none());
        }

        let entries = input.parse_comma_separated(|input| {
            let name = CustomIdent::parse(input, &["none"])?;
            let mut value = Vec::new();

            while !input.is_exhausted() {
                value.push(parse_content_item(
                    context, input, false, true, false, false, true, true,
                )?);
            }

            if value.is_empty() {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }

            Ok(generics::StringSetAssignment {
                name,
                value: value.into(),
            })
        })?;

        Ok(generics::StringSet(entries.into()))
    }
}
