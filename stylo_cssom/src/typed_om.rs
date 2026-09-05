use cssparser::{BasicParseErrorKind, Parser, ParserInput, SourcePosition, Token};
use style::parser::Parse;
use style_traits::ToCss;

use crate::style_fragment_parser::parse_style_fragment_with as parse_fragment_with;

macro_rules! typed_om_text_input {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name<'a>(&'a str);

        impl<'a> $name<'a> {
            #[must_use]
            pub const fn new(value: &'a str) -> Self {
                Self(value)
            }
        }
    };
}

typed_om_text_input!(TypedOmComputedNumericInput);
typed_om_text_input!(TypedOmUnparsedInput);
typed_om_text_input!(TypedOmListIterationsInput);
typed_om_text_input!(TypedOmBackgroundSizeInput);
typed_om_text_input!(TypedOmTextDecorationSkipInput);
typed_om_text_input!(TypedOmColorInput);
typed_om_text_input!(TypedOmImageInput);
typed_om_text_input!(TypedOmTransformInput);

#[derive(Clone, Copy, Debug)]
pub struct TypedOmFontStretchInput<'a> {
    keyword: &'a str,
    computed: &'a str,
}

impl<'a> TypedOmFontStretchInput<'a> {
    #[must_use]
    pub const fn new(keyword: &'a str, computed: &'a str) -> Self {
        Self { keyword, computed }
    }
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOmUnparsedValue {
    segments: Box<[TypedOmUnparsedSegment]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOmListIterations {
    items: Box<[String]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOmBackgroundSizeNumericValue(String);

impl TypedOmBackgroundSizeNumericValue {
    pub fn numeric_component_text(&self) -> &str {
        &self.0
    }
}

pub fn parse_typed_om_computed_numeric_value(
    input: TypedOmComputedNumericInput<'_>,
) -> Option<stylo_cssom_model::ComputedNumericValue> {
    fn parse_numeric<'i, 't>(
        input: &mut Parser<'i, 't>,
    ) -> Result<stylo_cssom_model::ComputedNumericValue, cssparser::ParseError<'i, ()>> {
        let token = input.next()?.clone();
        match token {
            Token::Number { value, .. } => Ok(stylo_cssom_model::ComputedNumericValue {
                value: value.into(),
                unit: "number".into(),
            }),
            Token::Percentage { unit_value, .. } => Ok(stylo_cssom_model::ComputedNumericValue {
                value: (unit_value * 100.0).into(),
                unit: "percent".into(),
            }),
            Token::Dimension { value, unit, .. } => Ok(stylo_cssom_model::ComputedNumericValue {
                value: value.into(),
                unit: unit.to_ascii_lowercase().into(),
            }),
            Token::Function(name) if name.eq_ignore_ascii_case("calc") => {
                input.parse_nested_block(parse_numeric_sum)
            },
            _ => Err(input.new_custom_error(())),
        }
    }

    fn parse_numeric_sum<'i, 't>(
        input: &mut Parser<'i, 't>,
    ) -> Result<stylo_cssom_model::ComputedNumericValue, cssparser::ParseError<'i, ()>> {
        let mut result = parse_numeric(input)?;
        while !input.is_exhausted() {
            let operator = match input.next()? {
                Token::Delim('+') => 1.0,
                Token::Delim('-') => -1.0,
                _ => return Err(input.new_custom_error(())),
            };
            let term = parse_numeric(input)?;
            if term.unit != result.unit {
                return Err(input.new_custom_error(()));
            }
            result.value += operator * term.value;
        }
        Ok(result)
    }

    let mut input = ParserInput::new(input.0);
    let mut parser = Parser::new(&mut input);
    parser.parse_entirely(parse_numeric).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedOmTransformDimensionality {
    TwoDimensional,
    ThreeDimensional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedOmSkewFunction {
    Both,
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedOmTransformComponent {
    Matrix2D([f64; 16]),
    Matrix3D([f64; 16]),
    Translate {
        coordinates: [String; 3],
        dimensionality: TypedOmTransformDimensionality,
    },
    Scale {
        coordinates: [String; 3],
        dimensionality: TypedOmTransformDimensionality,
    },
    Rotate {
        axes: [String; 3],
        angle: String,
        dimensionality: TypedOmTransformDimensionality,
    },
    Skew {
        angles: [String; 2],
        function: TypedOmSkewFunction,
    },
    Perspective(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedOmTextDecorationSkipKeyword {
    None,
    Objects,
    Edges,
    BoxDecoration,
    Spaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedOmFontStretchKeyword {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl TypedOmFontStretchKeyword {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UltraCondensed => "ultra-condensed",
            Self::ExtraCondensed => "extra-condensed",
            Self::Condensed => "condensed",
            Self::SemiCondensed => "semi-condensed",
            Self::Normal => "normal",
            Self::SemiExpanded => "semi-expanded",
            Self::Expanded => "expanded",
            Self::ExtraExpanded => "extra-expanded",
            Self::UltraExpanded => "ultra-expanded",
        }
    }

    const fn percentage(self) -> f32 {
        match self {
            Self::UltraCondensed => 0.5,
            Self::ExtraCondensed => 0.625,
            Self::Condensed => 0.75,
            Self::SemiCondensed => 0.875,
            Self::Normal => 1.0,
            Self::SemiExpanded => 1.125,
            Self::Expanded => 1.25,
            Self::ExtraExpanded => 1.5,
            Self::UltraExpanded => 2.0,
        }
    }
}

impl TypedOmTextDecorationSkipKeyword {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Objects => "objects",
            Self::Edges => "edges",
            Self::BoxDecoration => "box-decoration",
            Self::Spaces => "spaces",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedOmImageValue {
    source: Option<String>,
}

impl TypedOmImageValue {
    pub fn into_source(self) -> Option<String> {
        self.source
    }
}

impl TypedOmListIterations {
    fn new(items: Vec<String>) -> Option<Self> {
        (!items.is_empty()).then(|| Self {
            items: items.into(),
        })
    }

    pub fn items(&self) -> impl ExactSizeIterator<Item = &str> {
        self.items.iter().map(String::as_str)
    }
}

impl TypedOmUnparsedValue {
    pub fn segments(&self) -> impl ExactSizeIterator<Item = &TypedOmUnparsedSegment> {
        self.segments.iter()
    }

    #[must_use]
    pub fn contains_variable_reference(&self) -> bool {
        self.segments
            .iter()
            .any(|segment| matches!(segment, TypedOmUnparsedSegment::VariableReference(_)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedOmUnparsedSegment {
    String(String),
    VariableReference(TypedOmVariableReference),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOmVariableReference {
    name: TypedOmCustomPropertyName,
    fallback: Option<Box<TypedOmUnparsedValue>>,
}

impl TypedOmVariableReference {
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[must_use]
    pub fn fallback(&self) -> Option<&TypedOmUnparsedValue> {
        self.fallback.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedOmCustomPropertyName(String);

impl TypedOmCustomPropertyName {
    fn parse(name: String) -> Option<Self> {
        crate::is_valid_custom_property_name(&name).then_some(Self(name))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reify the component-value boundaries required by CSS Typed OM sections 5.3
/// and 5.4 without interpreting the remaining CSS tokens as property grammar.
pub fn parse_typed_om_unparsed_value(
    input: TypedOmUnparsedInput<'_>,
) -> Option<TypedOmUnparsedValue> {
    let css = input.0;
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parse_typed_om_component_values(&mut parser).ok()
}

/// Divide a comma-list grammar into its top-level iterations without treating
/// commas inside functions or blocks as list separators.
pub fn parse_typed_om_list_iterations(
    input: TypedOmListIterationsInput<'_>,
) -> Option<TypedOmListIterations> {
    let css = input.0;
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let items = parser
        .parse_entirely(|input| {
            input.parse_comma_separated(|item| {
                let start = item.position();
                consume_typed_om_list_iteration(item)?;
                let value = item.slice_from(start).trim();
                if value.is_empty() {
                    return Err(item.new_custom_error(()));
                }
                Ok(value.to_owned())
            })
        })
        .ok()?;
    TypedOmListIterations::new(items)
}

pub fn parse_typed_om_background_size_numeric_value(
    input: TypedOmBackgroundSizeInput<'_>,
) -> Option<TypedOmBackgroundSizeNumericValue> {
    let css = input.0;
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let value = parser
        .parse_entirely(|input| {
            input.skip_whitespace();
            let start = input.position();
            let token = next_valid_typed_om_component_token(input)?
                .ok_or_else(|| input.new_custom_error(()))?;
            if matches!(
                token,
                Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            ) {
                input.parse_nested_block(consume_typed_om_list_iteration)?;
            }
            let value = input.slice_from(start).trim().to_owned();
            input.expect_ident_matching("auto")?;
            Ok(value)
        })
        .ok()?;
    Some(TypedOmBackgroundSizeNumericValue(value))
}

/// Recover the single-keyword legacy shorthand from Stylo's computed
/// longhand tuple. The four trailing components are the initial values of the
/// newer skip longhands and therefore do not contribute another shorthand
/// keyword.
pub fn parse_typed_om_text_decoration_skip_keyword(
    input: TypedOmTextDecorationSkipInput<'_>,
) -> Option<TypedOmTextDecorationSkipKeyword> {
    let css = input.0;
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_entirely(|input| {
            let keyword = match input.expect_ident_cloned()?.to_ascii_lowercase().as_str() {
                "none" => TypedOmTextDecorationSkipKeyword::None,
                "objects" => TypedOmTextDecorationSkipKeyword::Objects,
                "edges" => TypedOmTextDecorationSkipKeyword::Edges,
                "box-decoration" => TypedOmTextDecorationSkipKeyword::BoxDecoration,
                "spaces" => TypedOmTextDecorationSkipKeyword::Spaces,
                _ => return Err(input.new_custom_error::<(), ()>(())),
            };
            input.expect_ident_matching("none")?;
            input.expect_ident_matching("none")?;
            input.expect_ident_matching("start")?;
            input.expect_ident_matching("end")?;
            Ok(keyword)
        })
        .ok()
}

/// Recover a retained font-stretch keyword only when it denotes the computed
/// percentage produced by CSS Fonts 4. This prevents stale association
/// metadata from overriding a different computed width.
pub fn parse_typed_om_font_stretch_keyword(
    input: TypedOmFontStretchInput<'_>,
) -> Option<TypedOmFontStretchKeyword> {
    let TypedOmFontStretchInput { keyword, computed } = input;
    let keyword = match keyword.to_ascii_lowercase().as_str() {
        "ultra-condensed" => TypedOmFontStretchKeyword::UltraCondensed,
        "extra-condensed" => TypedOmFontStretchKeyword::ExtraCondensed,
        "condensed" => TypedOmFontStretchKeyword::Condensed,
        "semi-condensed" => TypedOmFontStretchKeyword::SemiCondensed,
        "normal" => TypedOmFontStretchKeyword::Normal,
        "semi-expanded" => TypedOmFontStretchKeyword::SemiExpanded,
        "expanded" => TypedOmFontStretchKeyword::Expanded,
        "extra-expanded" => TypedOmFontStretchKeyword::ExtraExpanded,
        "ultra-expanded" => TypedOmFontStretchKeyword::UltraExpanded,
        _ => return None,
    };
    let mut input = ParserInput::new(computed);
    let mut parser = Parser::new(&mut input);
    let percentage = parser
        .parse_entirely(|input| -> Result<f32, cssparser::ParseError<'_, ()>> {
            Ok(input.expect_percentage()?)
        })
        .ok()?;
    ((percentage - keyword.percentage()).abs() <= f32::EPSILON).then_some(keyword)
}

fn consume_typed_om_list_iteration<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    loop {
        let Some(token) = next_valid_typed_om_component_token(input)? else {
            return Ok(());
        };
        match token {
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                input.parse_nested_block(consume_typed_om_list_iteration)?;
            },
            _ => {},
        }
    }
}

fn next_valid_typed_om_component_token<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<Option<Token<'i>>, cssparser::ParseError<'i, ()>> {
    let token = match input.next_including_whitespace_and_comments() {
        Ok(token) => token.clone(),
        Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if matches!(
        &token,
        Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket
            | Token::BadString(_)
            | Token::BadUrl(_)
    ) {
        return Err(input.new_unexpected_token_error(token));
    }
    Ok(Some(token))
}

fn parse_typed_om_component_values<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<TypedOmUnparsedValue, cssparser::ParseError<'i, ()>> {
    let mut segments = Vec::new();
    let mut raw_start = input.position();
    collect_typed_om_component_values(input, &mut raw_start, &mut segments)?;
    append_typed_om_string_segment(input, raw_start, input.position(), &mut segments);
    Ok(TypedOmUnparsedValue {
        segments: segments.into(),
    })
}

fn collect_typed_om_component_values<'i>(
    input: &mut Parser<'i, '_>,
    raw_start: &mut SourcePosition,
    segments: &mut Vec<TypedOmUnparsedSegment>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    loop {
        let token_start = input.position();
        let Some(token) = next_valid_typed_om_component_token(input)? else {
            return Ok(());
        };
        match token {
            Token::Function(name) if name.eq_ignore_ascii_case("var") => {
                append_typed_om_string_segment(input, *raw_start, token_start, segments);
                let reference = input.parse_nested_block(parse_typed_om_variable_reference)?;
                segments.push(TypedOmUnparsedSegment::VariableReference(reference));
                *raw_start = input.position();
            },
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                input.parse_nested_block(|nested| {
                    collect_typed_om_component_values(nested, raw_start, segments)
                })?;
            },
            _ => {},
        }
    }
}

fn append_typed_om_string_segment(
    input: &Parser<'_, '_>,
    start: SourcePosition,
    end: SourcePosition,
    segments: &mut Vec<TypedOmUnparsedSegment>,
) {
    if start != end {
        segments.push(TypedOmUnparsedSegment::String(
            input.slice(start..end).to_owned(),
        ));
    }
}

fn parse_typed_om_variable_reference<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<TypedOmVariableReference, cssparser::ParseError<'i, ()>> {
    let name = input.expect_ident_cloned()?.to_string();
    let name = TypedOmCustomPropertyName::parse(name).ok_or_else(|| input.new_custom_error(()))?;
    input.skip_whitespace();
    let fallback = if input.is_exhausted() {
        None
    } else {
        input.expect_comma()?;
        Some(Box::new(parse_typed_om_component_values(input)?))
    };
    Ok(TypedOmVariableReference { name, fallback })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypedOmColorChannel {
    Number(f64),
    Percent(f64),
    AngleDegrees(f64),
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedOmColorComponents {
    channels: [TypedOmColorChannel; 4],
}

impl TypedOmColorComponents {
    fn new(channels: [TypedOmColorChannel; 4]) -> Option<Self> {
        channels
            .iter()
            .all(|channel| match channel {
                TypedOmColorChannel::Number(value)
                | TypedOmColorChannel::Percent(value)
                | TypedOmColorChannel::AngleDegrees(value) => value.is_finite(),
                TypedOmColorChannel::None => true,
            })
            .then_some(Self { channels })
    }

    pub const fn channels(&self) -> &[TypedOmColorChannel; 4] {
        &self.channels
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedCssColorSerialization(String);

impl ValidatedCssColorSerialization {
    pub fn css_color_text(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypedOmParsedColor {
    Rgb(TypedOmColorComponents),
    Hsl(TypedOmColorComponents),
    Hwb(TypedOmColorComponents),
    Keyword(ValidatedCssColorSerialization),
    Unresolved(ValidatedCssColorSerialization),
    UnsupportedSubclass(ValidatedCssColorSerialization),
}

fn typed_om_color_channel(
    value: Option<f32>,
    make: impl FnOnce(f64) -> TypedOmColorChannel,
) -> TypedOmColorChannel {
    value.map_or(TypedOmColorChannel::None, |value| make(f64::from(value)))
}

pub fn parse_typed_om_color(input: TypedOmColorInput<'_>) -> Option<TypedOmParsedColor> {
    let css = input.0;
    use style::{color::ColorSpace, values::specified::Color as SpecifiedColor};

    let specified = parse_fragment_with(css, SpecifiedColor::parse)?;
    let serialization = ValidatedCssColorSerialization(specified.to_css_string());
    if matches!(specified, SpecifiedColor::System(_)) {
        return Some(TypedOmParsedColor::Keyword(ValidatedCssColorSerialization(
            serialization.0.to_ascii_lowercase(),
        )));
    }
    let Some(absolute) = specified.resolve_to_absolute() else {
        return Some(TypedOmParsedColor::Unresolved(serialization));
    };
    let channels = match absolute.color_space {
        ColorSpace::Srgb => {
            let rgb = |value: f64| {
                let value = value * 255.0;
                TypedOmColorChannel::Number(if absolute.is_legacy_syntax() {
                    value.round()
                } else {
                    value
                })
            };
            [
                typed_om_color_channel(absolute.c0(), rgb),
                typed_om_color_channel(absolute.c1(), rgb),
                typed_om_color_channel(absolute.c2(), rgb),
                typed_om_color_channel(absolute.alpha(), |value| {
                    TypedOmColorChannel::Percent(value * 100.0)
                }),
            ]
        },
        ColorSpace::Hsl | ColorSpace::Hwb => [
            typed_om_color_channel(absolute.c0(), TypedOmColorChannel::AngleDegrees),
            typed_om_color_channel(absolute.c1(), TypedOmColorChannel::Percent),
            typed_om_color_channel(absolute.c2(), TypedOmColorChannel::Percent),
            typed_om_color_channel(absolute.alpha(), |value| {
                TypedOmColorChannel::Percent(value * 100.0)
            }),
        ],
        _ => return Some(TypedOmParsedColor::UnsupportedSubclass(serialization)),
    };
    let channels = TypedOmColorComponents::new(channels)?;
    Some(match absolute.color_space {
        ColorSpace::Srgb => TypedOmParsedColor::Rgb(channels),
        ColorSpace::Hsl => TypedOmParsedColor::Hsl(channels),
        ColorSpace::Hwb => TypedOmParsedColor::Hwb(channels),
        _ => unreachable!("non-reified colour spaces returned above"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computed_numeric_values_reduce_same_unit_calc_sums() {
        assert_eq!(
            parse_typed_om_computed_numeric_value(TypedOmComputedNumericInput::new("calc(2 + 3)")),
            Some(stylo_cssom_model::ComputedNumericValue {
                value: 5.0,
                unit: "number".into(),
            })
        );
        assert_eq!(
            parse_typed_om_computed_numeric_value(TypedOmComputedNumericInput::new("calc(-3%)")),
            Some(stylo_cssom_model::ComputedNumericValue {
                value: -3.0,
                unit: "percent".into(),
            })
        );
        assert!(
            parse_typed_om_computed_numeric_value(TypedOmComputedNumericInput::new(
                "calc(1px + 1em)"
            ))
            .is_none()
        );
    }

    #[test]
    fn unparsed_values_preserve_component_and_variable_boundaries() {
        let value = parse_typed_om_unparsed_value(TypedOmUnparsedInput::new(
            "calc(42px + var(--foo, 15em) + var(--bar, var(--far) + 15px))",
        ))
        .expect("the component-value list must parse");
        let segments = value.segments().collect::<Vec<_>>();
        assert_eq!(segments.len(), 5);
        assert!(
            matches!(&segments[0], TypedOmUnparsedSegment::String(value) if value == "calc(42px + ")
        );
        let TypedOmUnparsedSegment::VariableReference(foo) = &segments[1] else {
            panic!("the first var() must remain a typed reference");
        };
        assert_eq!(foo.name(), "--foo");
        assert!(matches!(
            foo.fallback().and_then(|fallback| fallback.segments().next()),
            Some(TypedOmUnparsedSegment::String(value)) if value == " 15em"
        ));
        assert!(matches!(&segments[2], TypedOmUnparsedSegment::String(value) if value == " + "));
        let TypedOmUnparsedSegment::VariableReference(bar) = &segments[3] else {
            panic!("the second var() must remain a typed reference");
        };
        assert_eq!(bar.name(), "--bar");
        let bar_fallback = bar
            .fallback()
            .expect("the second var() must retain its fallback");
        let bar_segments = bar_fallback.segments().collect::<Vec<_>>();
        assert_eq!(bar_segments.len(), 3);
        assert!(matches!(&bar_segments[0], TypedOmUnparsedSegment::String(value) if value == " "));
        assert!(matches!(
            &bar_segments[1],
            TypedOmUnparsedSegment::VariableReference(reference)
                if reference.name() == "--far" && reference.fallback().is_none()
        ));
        assert!(
            matches!(&bar_segments[2], TypedOmUnparsedSegment::String(value) if value == " + 15px")
        );
        assert!(matches!(&segments[4], TypedOmUnparsedSegment::String(value) if value == ")"));
    }

    #[test]
    fn unparsed_values_reject_invalid_references_without_rewriting_strings() {
        assert!(parse_typed_om_unparsed_value(TypedOmUnparsedInput::new("var(foo)")).is_none());
        assert!(
            parse_typed_om_unparsed_value(TypedOmUnparsedInput::new("var(--foo trailing)"))
                .is_none()
        );
        let quoted = parse_typed_om_unparsed_value(TypedOmUnparsedInput::new(
            "url(\"var(--not-a-reference)\")",
        ))
        .expect("var() text inside a string must remain an ordinary component value");
        assert!(!quoted.contains_variable_reference());
        assert!(matches!(
            quoted.segments().next(),
            Some(TypedOmUnparsedSegment::String(value)) if value == "url(\"var(--not-a-reference)\")"
        ));
    }

    #[test]
    fn background_size_recovers_only_a_numeric_component_followed_by_auto() {
        assert_eq!(
            parse_typed_om_background_size_numeric_value(TypedOmBackgroundSizeInput::new(
                "calc(10px + 2%) auto"
            ))
            .expect("the numeric component must parse")
            .numeric_component_text(),
            "calc(10px + 2%)"
        );
        for invalid in ["10px 20px", "cover", "10px auto, 20px auto"] {
            assert!(
                parse_typed_om_background_size_numeric_value(TypedOmBackgroundSizeInput::new(
                    invalid
                ))
                .is_none()
            );
        }
    }

    #[test]
    fn text_decoration_skip_recovers_only_a_single_legacy_keyword() {
        assert_eq!(
            parse_typed_om_text_decoration_skip_keyword(TypedOmTextDecorationSkipInput::new(
                "objects none none start end"
            ))
            .map(TypedOmTextDecorationSkipKeyword::as_str),
            Some("objects")
        );
        assert!(
            parse_typed_om_text_decoration_skip_keyword(TypedOmTextDecorationSkipInput::new(
                "objects spaces none start end"
            ))
            .is_none()
        );
        assert!(
            parse_typed_om_text_decoration_skip_keyword(TypedOmTextDecorationSkipInput::new(
                "objects none none start"
            ))
            .is_none()
        );
    }

    #[test]
    fn font_stretch_keyword_must_match_its_computed_percentage() {
        assert_eq!(
            parse_typed_om_font_stretch_keyword(TypedOmFontStretchInput::new(
                "semi-condensed",
                "87.5%"
            ))
            .map(TypedOmFontStretchKeyword::as_str),
            Some("semi-condensed")
        );
        assert!(
            parse_typed_om_font_stretch_keyword(TypedOmFontStretchInput::new(
                "semi-condensed",
                "75%"
            ))
            .is_none()
        );
        assert!(
            parse_typed_om_font_stretch_keyword(TypedOmFontStretchInput::new("narrow", "87.5%"))
                .is_none()
        );
    }

    #[test]
    fn images_distinguish_valid_opaque_images_from_none() {
        assert!(
            parse_typed_om_image(TypedOmImageInput::new(
                "url(https://example.test/image.png)"
            ))
            .and_then(TypedOmImageValue::into_source)
            .is_some()
        );
        assert!(parse_typed_om_image(TypedOmImageInput::new("url(image.png)")).is_some());
        assert!(
            parse_typed_om_image(TypedOmImageInput::new("linear-gradient(red, blue)"))
                .is_some_and(|image| image.into_source().is_none())
        );
        assert!(parse_typed_om_image(TypedOmImageInput::new("none")).is_none());
    }

    #[test]
    fn list_iterations_split_only_top_level_commas() {
        let values = parse_typed_om_list_iterations(TypedOmListIterationsInput::new("1s, 2s"))
            .expect("the non-empty time list must parse");
        assert_eq!(values.items().collect::<Vec<_>>(), ["1s", "2s"]);
        let nested = parse_typed_om_list_iterations(TypedOmListIterationsInput::new(
            "cubic-bezier(0, 0, 1, 1), steps(2, end)",
        ))
        .expect("function commas must remain inside their iterations");
        assert_eq!(
            nested.items().collect::<Vec<_>>(),
            ["cubic-bezier(0, 0, 1, 1)", "steps(2, end)"]
        );
        assert!(parse_typed_om_list_iterations(TypedOmListIterationsInput::new("")).is_none());
        assert!(parse_typed_om_list_iterations(TypedOmListIterationsInput::new("1s,")).is_none());
    }

    #[test]
    fn transform_lists_have_non_empty_typed_components() {
        let components = parse_typed_om_transform_list(TypedOmTransformInput::new(
            "translate(50%, 12px) rotate3d(1, 2, 3, 45deg) perspective(calc(10em))",
        ))
        .expect("the transform list must parse into typed components");
        assert_eq!(components.len(), 3);
        assert!(matches!(
            &components[0],
            TypedOmTransformComponent::Translate {
                coordinates,
                dimensionality: TypedOmTransformDimensionality::TwoDimensional,
            } if coordinates == &["50%", "12px", "0px"]
        ));
        assert!(matches!(
            &components[1],
            TypedOmTransformComponent::Rotate {
                axes,
                angle,
                dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
            } if axes == &["1", "2", "3"] && angle == "45deg"
        ));
        assert!(matches!(
            &components[2],
            TypedOmTransformComponent::Perspective(Some(length)) if length == "calc(10em)"
        ));
        assert!(parse_typed_om_transform_list(TypedOmTransformInput::new("none")).is_none());
        assert!(
            parse_typed_om_transform_list(TypedOmTransformInput::new(
                "translate(10px,) rotate(1deg)"
            ))
            .is_none()
        );
        assert!(
            parse_typed_om_transform_list(TypedOmTransformInput::new(
                "matrix(sibling-index(), 2, 3, 4, 5, 6)"
            ))
            .is_none(),
            "tree-dependent matrix values cannot be represented by CSSMatrixComponent",
        );
    }

    #[test]
    fn colour_parser_separates_grammar_and_reification_kind() {
        let TypedOmParsedColor::Rgb(rgb) =
            parse_typed_om_color(TypedOmColorInput::new("#00bfff")).expect("hex colour must parse")
        else {
            panic!("hex colours must reify as RGB")
        };
        assert_eq!(
            rgb.channels(),
            &[
                TypedOmColorChannel::Number(0.0),
                TypedOmColorChannel::Number(191.0),
                TypedOmColorChannel::Number(255.0),
                TypedOmColorChannel::Percent(100.0),
            ]
        );
        let TypedOmParsedColor::Hsl(hsl) =
            parse_typed_om_color(TypedOmColorInput::new("hsl(195, 100%, 50%)"))
                .expect("HSL must parse")
        else {
            panic!("HSL must retain its function kind")
        };
        assert_eq!(hsl.channels()[0], TypedOmColorChannel::AngleDegrees(195.0));
        assert!(matches!(
            parse_typed_om_color(TypedOmColorInput::new("GrayText")),
            Some(TypedOmParsedColor::Keyword(value)) if value.css_color_text() == "graytext"
        ));
        assert!(matches!(
            parse_typed_om_color(TypedOmColorInput::new("currentcolor")),
            Some(TypedOmParsedColor::Unresolved(_))
        ));
        assert!(matches!(
            parse_typed_om_color(TypedOmColorInput::new("lab(50% 0 0)")),
            Some(TypedOmParsedColor::UnsupportedSubclass(_))
        ));
        for invalid in [
            "abcdef",
            "initial",
            "inherit",
            "unset",
            "revert",
            "revert-layer",
        ] {
            assert!(
                parse_typed_om_color(TypedOmColorInput::new(invalid)).is_none(),
                "{invalid} must remain a syntax error"
            );
        }
    }
}

pub fn parse_typed_om_image(input: TypedOmImageInput<'_>) -> Option<TypedOmImageValue> {
    let css = input.0;
    use style::values::generics::image::{GenericImageSrc, Image as StyloImage};
    let specified_value = parse_fragment_with(css, |context, input| {
        style::values::specified::Image::parse(context, input)
    })?;
    let source = match specified_value {
        StyloImage::Url(url) => {
            let resolved = url.as_str();
            if resolved.is_empty() {
                None
            } else {
                Some(resolved.to_owned())
            }
        },

        StyloImage::Image(payload) => {
            let Some(source) = payload.src else {
                return Some(TypedOmImageValue { source: None });
            };
            match source {
                GenericImageSrc::Url(url) | GenericImageSrc::String(url) => {
                    let resolved = url.as_str();
                    if resolved.is_empty() {
                        None
                    } else {
                        Some(resolved.to_owned())
                    }
                },
            }
        },

        StyloImage::None => return None,
        StyloImage::Gradient(_)
        | StyloImage::CrossFade(_)
        | StyloImage::ImageSet(_)
        | StyloImage::PaintWorklet(_)
        | StyloImage::LightDark(_) => None,
    };
    Some(TypedOmImageValue { source })
}

fn typed_om_matrix_2d(
    matrix: &style::values::generics::transform::Matrix<style::values::specified::Number>,
) -> Option<TypedOmTransformComponent> {
    Some(TypedOmTransformComponent::Matrix2D([
        f64::from(matrix.a.resolve()?),
        f64::from(matrix.b.resolve()?),
        0.0,
        0.0,
        f64::from(matrix.c.resolve()?),
        f64::from(matrix.d.resolve()?),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        f64::from(matrix.e.resolve()?),
        f64::from(matrix.f.resolve()?),
        0.0,
        1.0,
    ]))
}

fn typed_om_matrix_3d(
    matrix: &style::values::generics::transform::Matrix3D<style::values::specified::Number>,
) -> Option<TypedOmTransformComponent> {
    Some(TypedOmTransformComponent::Matrix3D([
        f64::from(matrix.m11.resolve()?),
        f64::from(matrix.m12.resolve()?),
        f64::from(matrix.m13.resolve()?),
        f64::from(matrix.m14.resolve()?),
        f64::from(matrix.m21.resolve()?),
        f64::from(matrix.m22.resolve()?),
        f64::from(matrix.m23.resolve()?),
        f64::from(matrix.m24.resolve()?),
        f64::from(matrix.m31.resolve()?),
        f64::from(matrix.m32.resolve()?),
        f64::from(matrix.m33.resolve()?),
        f64::from(matrix.m34.resolve()?),
        f64::from(matrix.m41.resolve()?),
        f64::from(matrix.m42.resolve()?),
        f64::from(matrix.m43.resolve()?),
        f64::from(matrix.m44.resolve()?),
    ]))
}

pub fn parse_typed_om_transform_list(
    input: TypedOmTransformInput<'_>,
) -> Option<Box<[TypedOmTransformComponent]>> {
    let css = input.0;
    use style::values::{
        generics::transform::PerspectiveFunction, specified::transform::TransformOperation,
    };

    let specified = parse_fragment_with(css, |context, input| {
        style::values::specified::Transform::parse(context, input)
    })?;
    let components = specified
        .0
        .iter()
        .map(|operation| {
            let component = match operation {
                TransformOperation::Matrix(matrix) => typed_om_matrix_2d(matrix)?,
                TransformOperation::Matrix3D(matrix) => typed_om_matrix_3d(matrix)?,
                TransformOperation::Translate(x, y) => TypedOmTransformComponent::Translate {
                    coordinates: [x.to_css_string(), y.to_css_string(), "0px".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::TranslateX(x) => TypedOmTransformComponent::Translate {
                    coordinates: [x.to_css_string(), "0px".to_owned(), "0px".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::TranslateY(y) => TypedOmTransformComponent::Translate {
                    coordinates: ["0px".to_owned(), y.to_css_string(), "0px".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::TranslateZ(z) => TypedOmTransformComponent::Translate {
                    coordinates: ["0px".to_owned(), "0px".to_owned(), z.to_css_string()],
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Translate3D(x, y, z) => TypedOmTransformComponent::Translate {
                    coordinates: [x.to_css_string(), y.to_css_string(), z.to_css_string()],
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Scale(x, y) => TypedOmTransformComponent::Scale {
                    coordinates: [x.to_css_string(), y.to_css_string(), "1".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::ScaleX(x) => TypedOmTransformComponent::Scale {
                    coordinates: [x.to_css_string(), "1".to_owned(), "1".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::ScaleY(y) => TypedOmTransformComponent::Scale {
                    coordinates: ["1".to_owned(), y.to_css_string(), "1".to_owned()],
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::ScaleZ(z) => TypedOmTransformComponent::Scale {
                    coordinates: ["1".to_owned(), "1".to_owned(), z.to_css_string()],
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Scale3D(x, y, z) => TypedOmTransformComponent::Scale {
                    coordinates: [x.to_css_string(), y.to_css_string(), z.to_css_string()],
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Rotate(angle) => TypedOmTransformComponent::Rotate {
                    axes: ["0".to_owned(), "0".to_owned(), "1".to_owned()],
                    angle: angle.to_css_string(),
                    dimensionality: TypedOmTransformDimensionality::TwoDimensional,
                },
                TransformOperation::RotateX(angle) => TypedOmTransformComponent::Rotate {
                    axes: ["1".to_owned(), "0".to_owned(), "0".to_owned()],
                    angle: angle.to_css_string(),
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::RotateY(angle) => TypedOmTransformComponent::Rotate {
                    axes: ["0".to_owned(), "1".to_owned(), "0".to_owned()],
                    angle: angle.to_css_string(),
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::RotateZ(angle) => TypedOmTransformComponent::Rotate {
                    axes: ["0".to_owned(), "0".to_owned(), "1".to_owned()],
                    angle: angle.to_css_string(),
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Rotate3D(x, y, z, angle) => TypedOmTransformComponent::Rotate {
                    axes: [x.to_css_string(), y.to_css_string(), z.to_css_string()],
                    angle: angle.to_css_string(),
                    dimensionality: TypedOmTransformDimensionality::ThreeDimensional,
                },
                TransformOperation::Skew(x, y) => TypedOmTransformComponent::Skew {
                    angles: [x.to_css_string(), y.to_css_string()],
                    function: TypedOmSkewFunction::Both,
                },
                TransformOperation::SkewX(x) => TypedOmTransformComponent::Skew {
                    angles: [x.to_css_string(), "0deg".to_owned()],
                    function: TypedOmSkewFunction::X,
                },
                TransformOperation::SkewY(y) => TypedOmTransformComponent::Skew {
                    angles: ["0deg".to_owned(), y.to_css_string()],
                    function: TypedOmSkewFunction::Y,
                },
                TransformOperation::Perspective(value) => {
                    TypedOmTransformComponent::Perspective(match value {
                        PerspectiveFunction::None => None,
                        PerspectiveFunction::Length(length) => Some(length.to_css_string()),
                    })
                },
                TransformOperation::InterpolateMatrix { .. }
                | TransformOperation::AccumulateMatrix { .. } => {
                    return None;
                },
            };
            Some(component)
        })
        .collect::<Option<Vec<_>>>()?;
    (!components.is_empty()).then(|| components.into_boxed_slice())
}

pub fn geometry_transform_is_context_free(serialization: &str) -> bool {
    let mut input = cssparser::ParserInput::new(serialization);
    let mut parser = cssparser::Parser::new(&mut input);
    parser
        .parse_entirely(validate_geometry_transform_tokens)
        .is_ok()
}

fn validate_geometry_transform_tokens<'i>(
    input: &mut cssparser::Parser<'i, '_>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    while !input.is_exhausted() {
        let token = input.next_including_whitespace_and_comments()?.clone();
        match token {
            cssparser::Token::Percentage { .. } => return Err(input.new_custom_error(())),
            cssparser::Token::Dimension { unit, .. }
                if !matches!(
                    unit.to_ascii_lowercase().as_str(),
                    "px" | "cm" | "mm" | "q" | "in" | "pt" | "pc" | "deg" | "grad" | "rad" | "turn"
                ) =>
            {
                return Err(input.new_custom_error(()));
            },
            cssparser::Token::Function(_)
            | cssparser::Token::ParenthesisBlock
            | cssparser::Token::SquareBracketBlock
            | cssparser::Token::CurlyBracketBlock => {
                input.parse_nested_block(validate_geometry_transform_tokens)?;
            },
            _ => {},
        }
    }
    Ok(())
}

pub fn css_transform_serializes_none(serialization: &str) -> bool {
    crate::registration::single_identifier(serialization)
        .is_some_and(|value| value.eq_ignore_ascii_case("none"))
}

pub fn computed_typed_value_from_serialization(
    serialization: &str,
) -> stylo_cssom_model::ComputedStyleValue {
    if let Some(value) = crate::typed_om::parse_typed_om_computed_numeric_value(
        crate::typed_om::TypedOmComputedNumericInput::new(serialization),
    ) {
        return stylo_cssom_model::ComputedStyleValue::Numeric(value);
    }
    let mut input = cssparser::ParserInput::new(serialization);
    let mut parser = cssparser::Parser::new(&mut input);
    if let Ok(keyword) = parser.expect_ident_cloned()
        && parser.is_exhausted()
    {
        return stylo_cssom_model::ComputedStyleValue::keyword(keyword.as_ref());
    }
    stylo_cssom_model::ComputedStyleValue::associated(serialization)
}
