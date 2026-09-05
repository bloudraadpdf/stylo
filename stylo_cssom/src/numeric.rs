macro_rules! typed_om_units {
    ($($variant:ident => ($factory:literal, $canonical:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub enum TypedOmUnit {
            $($variant),+
        }

        impl TypedOmUnit {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub fn parse(input: &str) -> Option<Self> {
                match input.to_ascii_lowercase().as_str() {
                    $($canonical => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub const fn factory_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $factory),+
                }
            }

            pub const fn canonical_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $canonical),+
                }
            }
        }
    };
}

typed_om_units! {
    Number => ("number", "number"),
    Percent => ("percent", "percent"),
    Cap => ("cap", "cap"),
    Ch => ("ch", "ch"),
    Em => ("em", "em"),
    Ex => ("ex", "ex"),
    Ic => ("ic", "ic"),
    Lh => ("lh", "lh"),
    Rcap => ("rcap", "rcap"),
    Rch => ("rch", "rch"),
    Rem => ("rem", "rem"),
    Rex => ("rex", "rex"),
    Ric => ("ric", "ric"),
    Rlh => ("rlh", "rlh"),
    Vw => ("vw", "vw"),
    Vh => ("vh", "vh"),
    Vi => ("vi", "vi"),
    Vb => ("vb", "vb"),
    Vmin => ("vmin", "vmin"),
    Vmax => ("vmax", "vmax"),
    Svw => ("svw", "svw"),
    Svh => ("svh", "svh"),
    Svi => ("svi", "svi"),
    Svb => ("svb", "svb"),
    Svmin => ("svmin", "svmin"),
    Svmax => ("svmax", "svmax"),
    Lvw => ("lvw", "lvw"),
    Lvh => ("lvh", "lvh"),
    Lvi => ("lvi", "lvi"),
    Lvb => ("lvb", "lvb"),
    Lvmin => ("lvmin", "lvmin"),
    Lvmax => ("lvmax", "lvmax"),
    Dvw => ("dvw", "dvw"),
    Dvh => ("dvh", "dvh"),
    Dvi => ("dvi", "dvi"),
    Dvb => ("dvb", "dvb"),
    Dvmin => ("dvmin", "dvmin"),
    Dvmax => ("dvmax", "dvmax"),
    Cqw => ("cqw", "cqw"),
    Cqh => ("cqh", "cqh"),
    Cqi => ("cqi", "cqi"),
    Cqb => ("cqb", "cqb"),
    Cqmin => ("cqmin", "cqmin"),
    Cqmax => ("cqmax", "cqmax"),
    Cm => ("cm", "cm"),
    Mm => ("mm", "mm"),
    Q => ("Q", "q"),
    In => ("in", "in"),
    Pt => ("pt", "pt"),
    Pc => ("pc", "pc"),
    Px => ("px", "px"),
    Deg => ("deg", "deg"),
    Grad => ("grad", "grad"),
    Rad => ("rad", "rad"),
    Turn => ("turn", "turn"),
    S => ("s", "s"),
    Ms => ("ms", "ms"),
    Hz => ("Hz", "hz"),
    KHz => ("kHz", "khz"),
    Dpi => ("dpi", "dpi"),
    Dpcm => ("dpcm", "dpcm"),
    Dppx => ("dppx", "dppx"),
    Fr => ("fr", "fr"),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssNumericBaseType {
    Length,
    Angle,
    Time,
    Frequency,
    Resolution,
    Flex,
    Percent,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalUnitDimension {
    AbsoluteLength,
    Angle,
    Time,
    Frequency,
    Resolution,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanonicalUnitConversion {
    pub dimension: CanonicalUnitDimension,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CssUnitKind {
    Number,
    Percent,
    ContextDependentLength,
    Canonical(CanonicalUnitConversion),
    Flex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CssNumericType {
    pub exponents: [i32; 7],
    pub percent_hint: Option<CssNumericBaseType>,
}

impl CssNumericType {
    pub const NUMBER: Self = Self {
        exponents: [0; 7],
        percent_hint: None,
    };

    pub fn for_base(base: CssNumericBaseType) -> Self {
        let mut result = Self::NUMBER;
        result.exponents[base.index()] = 1;
        result
    }

    pub fn add(self, other: Self) -> Option<Self> {
        if self == other {
            return Some(self);
        }
        if let Some(hint) = self.percent_hint.or(other.percent_hint) {
            if self.percent_hint.is_some()
                && other.percent_hint.is_some()
                && self.percent_hint != other.percent_hint
            {
                return None;
            }
            let left = self.applying_percent_hint(hint)?;
            let right = other.applying_percent_hint(hint)?;
            if left.exponents == right.exponents {
                return Some(left);
            }
        }
        if let Some(result) = self.add_percent_to_dimension(other) {
            return Some(result);
        }
        if let Some(result) = other.add_percent_to_dimension(self) {
            return Some(result);
        }
        match (self.percent_hint, other.percent_hint) {
            (Some(left), Some(right)) if left == right && self.exponents == other.exponents => {
                Some(self)
            },
            (Some(_), None) if self.exponents == other.exponents => Some(self),
            (None, Some(_)) if self.exponents == other.exponents => Some(other),
            _ => None,
        }
    }

    pub fn applying_percent_hint(self, hint: CssNumericBaseType) -> Option<Self> {
        if self.percent_hint.is_some_and(|existing| existing != hint) {
            return None;
        }
        let mut exponents = self.exponents;
        let percent = exponents[CssNumericBaseType::Percent.index()];
        exponents[CssNumericBaseType::Percent.index()] = 0;
        exponents[hint.index()] = exponents[hint.index()].checked_add(percent)?;
        Some(Self {
            exponents,
            percent_hint: Some(hint),
        })
    }

    pub fn multiply(self, other: Self) -> Option<Self> {
        let percent_hint = match (self.percent_hint, other.percent_hint) {
            (Some(left), Some(right)) if left != right => return None,
            (Some(hint), _) | (_, Some(hint)) => Some(hint),
            (None, None) => None,
        };
        let mut exponents = [0; 7];
        for (index, exponent) in exponents.iter_mut().enumerate() {
            *exponent = self.exponents[index].checked_add(other.exponents[index])?;
        }
        Some(Self {
            exponents,
            percent_hint,
        })
    }

    pub fn inverted(self) -> Option<Self> {
        let mut exponents = self.exponents;
        for exponent in &mut exponents {
            *exponent = exponent.checked_neg()?;
        }
        Some(Self {
            exponents,
            percent_hint: self.percent_hint,
        })
    }

    pub fn add_percent_to_dimension(self, dimension: Self) -> Option<Self> {
        if self != Self::for_base(CssNumericBaseType::Percent) || dimension.percent_hint.is_some() {
            return None;
        }
        let base = CssNumericBaseType::ALL.iter().copied().find(|base| {
            *base != CssNumericBaseType::Percent && dimension == Self::for_base(*base)
        })?;
        Some(Self {
            exponents: dimension.exponents,
            percent_hint: Some(base),
        })
    }

    pub fn property_grammar_probe(self) -> Option<&'static str> {
        if self == Self::NUMBER {
            return Some("0.5");
        }
        if self.percent_hint.is_none() {
            return CssNumericBaseType::ALL.iter().copied().find_map(|base| {
                (self == Self::for_base(base)).then_some(match base {
                    CssNumericBaseType::Length => "0.5px",
                    CssNumericBaseType::Angle => "0.5deg",
                    CssNumericBaseType::Time => "0.5s",
                    CssNumericBaseType::Frequency => "0.5hz",
                    CssNumericBaseType::Resolution => "0.5dppx",
                    CssNumericBaseType::Flex => "0.5fr",
                    CssNumericBaseType::Percent => "0.5%",
                })
            });
        }
        let hint = self.percent_hint?;
        (self.exponents == Self::for_base(hint).exponents).then_some(match hint {
            CssNumericBaseType::Length => "calc(0.5px + 0.5%)",
            CssNumericBaseType::Angle => "calc(0.5deg + 0.5%)",
            CssNumericBaseType::Time => "calc(0.5s + 0.5%)",
            CssNumericBaseType::Frequency => "calc(0.5hz + 0.5%)",
            CssNumericBaseType::Resolution => "calc(0.5dppx + 0.5%)",
            CssNumericBaseType::Flex => "calc(0.5fr + 0.5%)",
            CssNumericBaseType::Percent => "0.5%",
        })
    }
}

impl CssNumericBaseType {
    pub const ALL: &'static [Self] = &[
        Self::Length,
        Self::Angle,
        Self::Time,
        Self::Frequency,
        Self::Resolution,
        Self::Flex,
        Self::Percent,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Length => 0,
            Self::Angle => 1,
            Self::Time => 2,
            Self::Frequency => 3,
            Self::Resolution => 4,
            Self::Flex => 5,
            Self::Percent => 6,
        }
    }

    pub const fn css_name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Angle => "angle",
            Self::Time => "time",
            Self::Frequency => "frequency",
            Self::Resolution => "resolution",
            Self::Flex => "flex",
            Self::Percent => "percent",
        }
    }
}

impl CanonicalUnitDimension {
    pub const fn numeric_base_type(self) -> CssNumericBaseType {
        match self {
            Self::AbsoluteLength => CssNumericBaseType::Length,
            Self::Angle => CssNumericBaseType::Angle,
            Self::Time => CssNumericBaseType::Time,
            Self::Frequency => CssNumericBaseType::Frequency,
            Self::Resolution => CssNumericBaseType::Resolution,
        }
    }

    pub const fn canonical_unit(self) -> TypedOmUnit {
        match self {
            Self::AbsoluteLength => TypedOmUnit::Px,
            Self::Angle => TypedOmUnit::Deg,
            Self::Time => TypedOmUnit::S,
            Self::Frequency => TypedOmUnit::Hz,
            Self::Resolution => TypedOmUnit::Dppx,
        }
    }
}

impl TypedOmUnit {
    pub fn kind(self) -> CssUnitKind {
        let conversion = match self {
            Self::Number => return CssUnitKind::Number,
            Self::Percent => return CssUnitKind::Percent,
            Self::Cap
            | Self::Ch
            | Self::Em
            | Self::Ex
            | Self::Ic
            | Self::Lh
            | Self::Rcap
            | Self::Rch
            | Self::Rem
            | Self::Rex
            | Self::Ric
            | Self::Rlh
            | Self::Vw
            | Self::Vh
            | Self::Vi
            | Self::Vb
            | Self::Vmin
            | Self::Vmax
            | Self::Svw
            | Self::Svh
            | Self::Svi
            | Self::Svb
            | Self::Svmin
            | Self::Svmax
            | Self::Lvw
            | Self::Lvh
            | Self::Lvi
            | Self::Lvb
            | Self::Lvmin
            | Self::Lvmax
            | Self::Dvw
            | Self::Dvh
            | Self::Dvi
            | Self::Dvb
            | Self::Dvmin
            | Self::Dvmax
            | Self::Cqw
            | Self::Cqh
            | Self::Cqi
            | Self::Cqb
            | Self::Cqmin
            | Self::Cqmax => return CssUnitKind::ContextDependentLength,
            Self::Cm => (CanonicalUnitDimension::AbsoluteLength, 96.0 / 2.54),
            Self::Mm => (CanonicalUnitDimension::AbsoluteLength, 96.0 / 25.4),
            Self::Q => (CanonicalUnitDimension::AbsoluteLength, 96.0 / 101.6),
            Self::In => (CanonicalUnitDimension::AbsoluteLength, 96.0),
            Self::Pt => (CanonicalUnitDimension::AbsoluteLength, 96.0 / 72.0),
            Self::Pc => (CanonicalUnitDimension::AbsoluteLength, 16.0),
            Self::Px => (CanonicalUnitDimension::AbsoluteLength, 1.0),
            Self::Deg => (CanonicalUnitDimension::Angle, 1.0),
            Self::Grad => (CanonicalUnitDimension::Angle, 0.9),
            Self::Rad => (CanonicalUnitDimension::Angle, 180.0 / std::f64::consts::PI),
            Self::Turn => (CanonicalUnitDimension::Angle, 360.0),
            Self::S => (CanonicalUnitDimension::Time, 1.0),
            Self::Ms => (CanonicalUnitDimension::Time, 0.001),
            Self::Hz => (CanonicalUnitDimension::Frequency, 1.0),
            Self::KHz => (CanonicalUnitDimension::Frequency, 1000.0),
            Self::Dpi => (CanonicalUnitDimension::Resolution, 1.0 / 96.0),
            Self::Dpcm => (CanonicalUnitDimension::Resolution, 2.54 / 96.0),
            Self::Dppx => (CanonicalUnitDimension::Resolution, 1.0),
            Self::Fr => return CssUnitKind::Flex,
        };
        CssUnitKind::Canonical(CanonicalUnitConversion {
            dimension: conversion.0,
            scale: conversion.1,
        })
    }

    pub fn numeric_base_type(self) -> Option<CssNumericBaseType> {
        match self.kind() {
            CssUnitKind::Number => None,
            CssUnitKind::Percent => Some(CssNumericBaseType::Percent),
            CssUnitKind::ContextDependentLength => Some(CssNumericBaseType::Length),
            CssUnitKind::Canonical(conversion) => Some(conversion.dimension.numeric_base_type()),
            CssUnitKind::Flex => Some(CssNumericBaseType::Flex),
        }
    }

    pub fn canonical_conversion(self) -> Option<CanonicalUnitConversion> {
        match self.kind() {
            CssUnitKind::Number
            | CssUnitKind::Percent
            | CssUnitKind::ContextDependentLength
            | CssUnitKind::Flex => None,
            CssUnitKind::Canonical(conversion) => Some(conversion),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NonEmptyCssList<T> {
    pub first: T,
    pub remaining: Vec<T>,
}

impl<T> NonEmptyCssList<T> {
    pub fn from_vec(values: Vec<T>) -> Option<Self> {
        let mut values = values.into_iter();
        Some(Self {
            first: values.next()?,
            remaining: values.collect(),
        })
    }

    pub fn into_vec(self) -> Vec<T> {
        std::iter::once(self.first).chain(self.remaining).collect()
    }

    pub fn into_single(self) -> Option<T> {
        self.remaining.is_empty().then_some(self.first)
    }

    pub fn len(&self) -> usize {
        1 + self.remaining.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.first).chain(&self.remaining)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpecifiedCssUnitValue {
    pub value: f64,
    pub unit: TypedOmUnit,
}

#[derive(Clone, Debug)]
pub enum SpecifiedCssNumericValue {
    Unit(SpecifiedCssUnitValue),
    Sum(NonEmptyCssList<Box<Self>>),
    Product(NonEmptyCssList<Box<Self>>),
    Negate(Box<Self>),
    Invert(Box<Self>),
    Min(NonEmptyCssList<Box<Self>>),
    Max(NonEmptyCssList<Box<Self>>),
    Clamp {
        lower: Box<Self>,
        value: Box<Self>,
        upper: Box<Self>,
    },
}

impl SpecifiedCssNumericValue {
    pub fn numeric_type(&self) -> Option<CssNumericType> {
        match self {
            Self::Unit(value) => Some(match value.unit.numeric_base_type() {
                Some(base) => CssNumericType::for_base(base),
                None => CssNumericType::NUMBER,
            }),
            Self::Sum(values) | Self::Min(values) | Self::Max(values) => {
                fold_specified_numeric_types(values, CssNumericType::add)
            },
            Self::Product(values) => fold_specified_numeric_types(values, CssNumericType::multiply),
            Self::Negate(value) => value.numeric_type(),
            Self::Invert(value) => value.numeric_type()?.inverted(),
            Self::Clamp {
                lower,
                value,
                upper,
            } => lower
                .numeric_type()?
                .add(value.numeric_type()?)?
                .add(upper.numeric_type()?),
        }
    }

    pub fn is_zero_percentage(&self) -> bool {
        match self {
            Self::Unit(value) => value.unit == TypedOmUnit::Percent && value.value == 0.0,
            Self::Sum(values) => values.iter().all(|value| value.is_zero_percentage()),
            Self::Negate(value) => value.is_zero_percentage(),
            Self::Product(_)
            | Self::Invert(_)
            | Self::Min(_)
            | Self::Max(_)
            | Self::Clamp { .. } => false,
        }
    }
}

pub fn fold_specified_numeric_types(
    values: &NonEmptyCssList<Box<SpecifiedCssNumericValue>>,
    combine: fn(CssNumericType, CssNumericType) -> Option<CssNumericType>,
) -> Option<CssNumericType> {
    let mut values = values.iter();
    let mut result = values.next()?.numeric_type()?;
    for value in values {
        result = combine(result, value.numeric_type()?)?;
    }
    Some(result)
}

pub fn parse_specified_css_numeric_value(source: &str) -> Option<SpecifiedCssNumericValue> {
    let mut input = cssparser::ParserInput::new(source);
    let mut parser = cssparser::Parser::new(&mut input);
    let value = parser.parse_entirely(parse_css_numeric_component).ok()?;
    value.numeric_type()?;
    Some(value)
}

pub fn parse_css_numeric_component<'i>(
    input: &mut cssparser::Parser<'i, '_>,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    parse_css_numeric_primary(input, false)
}

pub fn parse_css_numeric_primary<'i>(
    input: &mut cssparser::Parser<'i, '_>,
    allow_parentheses: bool,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    let token = input.next()?.clone();
    match token {
        cssparser::Token::Number { value, .. } => {
            Ok(specified_unit(f64::from(value), TypedOmUnit::Number))
        },
        cssparser::Token::Percentage {
            unit_value,
            int_value,
            ..
        } => Ok(specified_unit(
            int_value.map_or_else(|| f64::from(unit_value) * 100.0, f64::from),
            TypedOmUnit::Percent,
        )),
        cssparser::Token::Dimension { value, unit, .. } => {
            let Some(unit) = TypedOmUnit::parse(unit.as_ref()) else {
                return Err(input.new_custom_error(()));
            };
            Ok(specified_unit(f64::from(value), unit))
        },
        cssparser::Token::Function(name) => parse_css_numeric_function(input, name.as_ref()),
        cssparser::Token::ParenthesisBlock if allow_parentheses => {
            input.parse_nested_block(|nested| {
                let result = parse_css_numeric_sum(nested)?;
                nested.expect_exhausted()?;
                Ok(result)
            })
        },
        _ => Err(input.new_custom_error(())),
    }
}

pub fn parse_css_numeric_function<'i>(
    input: &mut cssparser::Parser<'i, '_>,
    name: &str,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    if name.eq_ignore_ascii_case("calc") {
        return input.parse_nested_block(|nested| {
            let result = parse_css_numeric_sum(nested)?;
            nested.expect_exhausted()?;
            Ok(match result {
                SpecifiedCssNumericValue::Sum(_) | SpecifiedCssNumericValue::Product(_) => result,
                value => specified_sum(vec![value]).ok_or_else(|| nested.new_custom_error(()))?,
            })
        });
    }
    if name.eq_ignore_ascii_case("min") || name.eq_ignore_ascii_case("max") {
        return input.parse_nested_block(|nested| {
            let values = parse_css_numeric_arguments(nested)?;
            Ok(if name.eq_ignore_ascii_case("min") {
                SpecifiedCssNumericValue::Min(values)
            } else {
                SpecifiedCssNumericValue::Max(values)
            })
        });
    }
    if name.eq_ignore_ascii_case("clamp") {
        return input.parse_nested_block(|nested| {
            let values = parse_css_numeric_arguments(nested)?.into_vec();
            let [lower, value, upper]: [Box<SpecifiedCssNumericValue>; 3] =
                values.try_into().map_err(|_| nested.new_custom_error(()))?;
            Ok(SpecifiedCssNumericValue::Clamp {
                lower,
                value,
                upper,
            })
        });
    }
    Err(input.new_custom_error(()))
}

pub fn parse_css_numeric_arguments<'i>(
    input: &mut cssparser::Parser<'i, '_>,
) -> Result<NonEmptyCssList<Box<SpecifiedCssNumericValue>>, cssparser::ParseError<'i, ()>> {
    let mut values = vec![Box::new(parse_css_numeric_sum(input)?)];
    while !input.is_exhausted() {
        input.expect_comma()?;
        values.push(Box::new(parse_css_numeric_sum(input)?));
    }
    NonEmptyCssList::from_vec(values).ok_or_else(|| input.new_custom_error(()))
}

pub fn parse_css_numeric_sum<'i>(
    input: &mut cssparser::Parser<'i, '_>,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    parse_css_numeric_binary(input, CssNumericBinaryPrecedence::Sum)
}

pub fn parse_css_numeric_product<'i>(
    input: &mut cssparser::Parser<'i, '_>,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    parse_css_numeric_binary(input, CssNumericBinaryPrecedence::Product)
}

#[derive(Clone, Copy)]
pub enum CssNumericBinaryPrecedence {
    Sum,
    Product,
}

pub fn parse_css_numeric_binary<'i>(
    input: &mut cssparser::Parser<'i, '_>,
    precedence: CssNumericBinaryPrecedence,
) -> Result<SpecifiedCssNumericValue, cssparser::ParseError<'i, ()>> {
    let parse_operand = |input: &mut cssparser::Parser<'i, '_>| match precedence {
        CssNumericBinaryPrecedence::Sum => parse_css_numeric_product(input),
        CssNumericBinaryPrecedence::Product => parse_css_numeric_primary(input, true),
    };
    let mut values = vec![parse_operand(input)?];
    while let Ok(operator) = input.try_parse(|input| {
        let location = input.current_source_location();
        match (precedence, input.next()?.clone()) {
            (CssNumericBinaryPrecedence::Sum, cssparser::Token::Delim('+'))
            | (CssNumericBinaryPrecedence::Product, cssparser::Token::Delim('*')) => Ok(false),
            (CssNumericBinaryPrecedence::Sum, cssparser::Token::Delim('-'))
            | (CssNumericBinaryPrecedence::Product, cssparser::Token::Delim('/')) => Ok(true),
            (_, token) => Err(location.new_unexpected_token_error::<()>(token)),
        }
    }) {
        let value = parse_operand(input)?;
        values.push(match (precedence, operator) {
            (CssNumericBinaryPrecedence::Sum, true) => {
                SpecifiedCssNumericValue::Negate(Box::new(value))
            },
            (CssNumericBinaryPrecedence::Product, true) => {
                SpecifiedCssNumericValue::Invert(Box::new(value))
            },
            (CssNumericBinaryPrecedence::Sum | CssNumericBinaryPrecedence::Product, false) => value,
        });
    }
    if values.len() == 1 {
        return values.pop().ok_or_else(|| input.new_custom_error(()));
    }
    match precedence {
        CssNumericBinaryPrecedence::Sum => {
            specified_sum(values).ok_or_else(|| input.new_custom_error(()))
        },
        CssNumericBinaryPrecedence::Product => {
            let values = values.into_iter().map(Box::new).collect();
            Ok(SpecifiedCssNumericValue::Product(
                NonEmptyCssList::from_vec(values).ok_or_else(|| input.new_custom_error(()))?,
            ))
        },
    }
}

pub fn specified_sum(values: Vec<SpecifiedCssNumericValue>) -> Option<SpecifiedCssNumericValue> {
    let mut result: Vec<Box<SpecifiedCssNumericValue>> = Vec::new();
    for value in values {
        let SpecifiedCssNumericValue::Unit(unit) = value else {
            result.push(Box::new(value));
            continue;
        };
        let CssUnitKind::Canonical(conversion) = unit.unit.kind() else {
            result.push(Box::new(SpecifiedCssNumericValue::Unit(unit)));
            continue;
        };
        if let Some(existing) = result.iter_mut().find_map(|value| match value.as_mut() {
            SpecifiedCssNumericValue::Unit(existing)
                if existing
                    .unit
                    .canonical_conversion()
                    .is_some_and(|candidate| candidate.dimension == conversion.dimension) =>
            {
                Some(existing)
            },
            _ => None,
        }) {
            let existing_conversion = existing.unit.canonical_conversion()?;
            existing.value =
                existing.value * existing_conversion.scale + unit.value * conversion.scale;
            existing.unit = conversion.dimension.canonical_unit();
        } else {
            result.push(Box::new(SpecifiedCssNumericValue::Unit(unit)));
        }
    }
    Some(SpecifiedCssNumericValue::Sum(NonEmptyCssList::from_vec(
        result,
    )?))
}

pub const fn specified_unit(value: f64, unit: TypedOmUnit) -> SpecifiedCssNumericValue {
    SpecifiedCssNumericValue::Unit(SpecifiedCssUnitValue { value, unit })
}
