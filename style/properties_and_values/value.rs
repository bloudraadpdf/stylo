/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Parsing for registered custom properties.

use std::borrow::Cow;
use std::fmt::{self, Write};

use super::{
    registry::PropertyRegistrationData,
    syntax::{
        data_type::DataType, Component as SyntaxComponent, ComponentName, Descriptor, Multiplier,
    },
};
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::properties;
use crate::rule_tree::CascadeLevel;
use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
use crate::stylist::Stylist;
use crate::values::{
    animated::{self, Animate, Procedure},
    computed::{self, ToComputedValue},
    generics::transform::Transform,
    specified, CustomIdent,
};
use crate::{custom_properties::ComputedValue as ComputedPropertyValue, dom::AttributeTracker};
use cssparser::{BasicParseErrorKind, ParseErrorKind, Parser as CSSParser, TokenSerializationType};
use selectors::matching::QuirksMode;
use servo_arc::Arc;
use smallvec::SmallVec;
use style_traits::{
    owned_str::OwnedStr, CssWriter, ParseError as StyleParseError, ParsingMode,
    StyleParseErrorKind, ToCss,
};

/// A single component of the computed value.
pub type ComputedValueComponent = GenericValueComponent<
    computed::Length,
    computed::Number,
    computed::Percentage,
    computed::LengthPercentage,
    computed::Color,
    computed::Image,
    computed::url::ComputedUrl,
    computed::Integer,
    computed::Angle,
    computed::Time,
    computed::Resolution,
    SingleTransformFunction<computed::TransformOperation>,
    computed::Transform,
>;

/// A single component of the specified value.
pub type SpecifiedValueComponent = GenericValueComponent<
    specified::Length,
    specified::Number,
    specified::Percentage,
    specified::LengthPercentage,
    specified::Color,
    specified::Image,
    specified::url::SpecifiedUrl,
    specified::Integer,
    specified::Angle,
    specified::Time,
    specified::Resolution,
    SingleTransformFunction<specified::TransformOperation>,
    specified::Transform,
>;

impl<L, N, P, LP, C, Image, U, Integer, A, T, R, TF, TL>
    GenericValueComponent<L, N, P, LP, C, Image, U, Integer, A, T, R, TF, TL>
{
    fn serialization_types(&self) -> (TokenSerializationType, TokenSerializationType) {
        let first_token_type = match self {
            Self::Length(_) | Self::Angle(_) | Self::Time(_) | Self::Resolution(_) => {
                TokenSerializationType::Dimension
            },
            Self::Number(_) | Self::Integer(_) => TokenSerializationType::Number,
            Self::Percentage(_) | Self::LengthPercentage(_) => TokenSerializationType::Percentage,
            Self::Color(_)
            | Self::Image(_)
            | Self::Url(_)
            | Self::TransformFunction(_)
            | Self::TransformList(_) => TokenSerializationType::Function,
            Self::CustomIdent(_) => TokenSerializationType::Ident,
            Self::String(_) => TokenSerializationType::Other,
        };
        let last_token_type = if first_token_type == TokenSerializationType::Function {
            TokenSerializationType::Other
        } else {
            first_token_type
        };
        (first_token_type, last_token_type)
    }
}

/// A generic enum used for both specified value components and computed value components.
#[derive(
    Animate, Clone, ToCss, ToComputedValue, ToResolvedValue, Debug, MallocSizeOf, PartialEq, ToShmem,
)]
#[animation(no_bound(Image, Url))]
pub enum GenericValueComponent<
    Length,
    Number,
    Percentage,
    LengthPercentage,
    Color,
    Image,
    Url,
    Integer,
    Angle,
    Time,
    Resolution,
    TransformFunction,
    TransformList,
> {
    /// A <length> value
    Length(Length),
    /// A <number> value
    Number(Number),
    /// A <percentage> value
    Percentage(Percentage),
    /// A <length-percentage> value
    LengthPercentage(LengthPercentage),
    /// A <color> value
    Color(Color),
    /// An <image> value
    #[animation(error)]
    Image(Image),
    /// A <url> value
    #[animation(error)]
    Url(Url),
    /// An <integer> value
    Integer(Integer),
    /// An <angle> value
    Angle(Angle),
    /// A <time> value
    Time(Time),
    /// A <resolution> value
    Resolution(Resolution),
    /// A <transform-function> value
    TransformFunction(TransformFunction),
    /// A <custom-ident> value
    #[animation(error)]
    CustomIdent(CustomIdent),
    /// A <transform-list> value, equivalent to <transform-function>+
    TransformList(TransformList),
    /// A <string> value
    #[animation(error)]
    String(OwnedStr),
}

/// A single <transform-function> value.
#[derive(
    Clone, ToComputedValue, ToResolvedValue, ToCss, Debug, MallocSizeOf, PartialEq, ToShmem,
)]
pub struct SingleTransformFunction<Operation>(Operation);

/// <https://drafts.css-houdini.org/css-properties-values-api-1/#animation-behavior-of-custom-properties>
impl Animate for SingleTransformFunction<computed::TransformOperation> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        // Concatenation cannot give a single function, so addition accumulates.
        let procedure = match procedure {
            Procedure::Add => Procedure::Accumulate { count: 1 },
            procedure => procedure,
        };
        let list = |function: &Self| Transform(vec![function.0.clone()].into());
        match &*list(self).animate(&list(other), procedure)?.0 {
            [operation] => Ok(Self(operation.clone())),
            _ => Err(()),
        }
    }
}

/// A list of component values, including the list's multiplier.
#[derive(Clone, ToComputedValue, ToResolvedValue, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct ComponentList<Component> {
    /// Multiplier
    pub multiplier: Multiplier,
    /// The list of components contained.
    pub components: crate::OwnedSlice<Component>,
}

impl<Component: Animate> Animate for ComponentList<Component> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self.multiplier != other.multiplier {
            return Err(());
        }
        let components = animated::lists::by_computed_value::animate(
            &self.components,
            &other.components,
            procedure,
        )?;
        Ok(Self {
            multiplier: self.multiplier,
            components,
        })
    }
}

impl<Component: ToCss> ToCss for ComponentList<Component> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let mut iter = self.components.iter();
        let Some(first) = iter.next() else {
            return Ok(());
        };
        first.to_css(dest)?;

        // The separator implied by the multiplier for this list.
        let separator = match self.multiplier {
            // <https://drafts.csswg.org/cssom-1/#serialize-a-whitespace-separated-list>
            Multiplier::Space => " ",
            // <https://drafts.csswg.org/cssom-1/#serialize-a-comma-separated-list>
            Multiplier::Comma => ", ",
        };
        for component in iter {
            dest.write_str(separator)?;
            component.to_css(dest)?;
        }
        Ok(())
    }
}

/// A struct for a single specified registered custom property value that includes its original URL
// data so the value can be uncomputed later.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToComputedValue, ToResolvedValue, ToShmem,
)]
pub struct Value<Component> {
    /// The registered custom property value.
    pub(crate) v: ValueInner<Component>,
    /// The URL data of the registered custom property from before it was computed. This is
    /// necessary to uncompute registered custom properties.
    #[css(skip)]
    url_data: UrlExtraData,
    #[css(skip)]
    pub(crate) attr_tainted: bool,
}

impl<Component: Animate + Clone> Animate for Value<Component> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let v = match procedure {
            Procedure::Interpolate { progress } if progress == 0.0 => self.v.clone(),
            Procedure::Interpolate { progress } if progress == 1.0 => other.v.clone(),
            _ => self
                .v
                .animate(&other.v, procedure)
                .or_else(|()| match procedure {
                    Procedure::Interpolate { progress } => Ok(if progress < 0.5 {
                        self.v.clone()
                    } else {
                        other.v.clone()
                    }),
                    _ => Err(()),
                })?,
        };
        Ok(Value {
            v,
            url_data: self.url_data.clone(),
            attr_tainted: self.attr_tainted || other.attr_tainted,
        })
    }
}

impl<Component> Value<Component> {
    /// The typed value retained by this registration.
    pub fn inner(&self) -> &ValueInner<Component> {
        &self.v
    }

    /// Creates a new registered custom property value.
    pub fn new(v: ValueInner<Component>, url_data: UrlExtraData) -> Self {
        let attr_tainted = matches!(&v, ValueInner::Universal(value) if value.is_attr_tainted());
        Self {
            v,
            url_data,
            attr_tainted,
        }
    }

    /// Creates a new registered custom property value presumed to have universal syntax.
    pub fn universal(var: Arc<ComputedPropertyValue>) -> Self {
        let url_data = var.url_data.clone();
        let attr_tainted = var.is_attr_tainted();
        let v = ValueInner::Universal(var);
        Self {
            v,
            url_data,
            attr_tainted,
        }
    }

    pub(crate) fn with_attr_taint(mut self, tainted: bool) -> Self {
        self.attr_tainted |= tainted;
        self
    }
}

impl<L, N, P, LP, C, Image, U, Integer, A, T, R, TF, TL>
    Value<GenericValueComponent<L, N, P, LP, C, Image, U, Integer, A, T, R, TF, TL>>
where
    Self: ToCss,
{
    pub(crate) fn contains_url(&self) -> bool {
        match &self.v {
            ValueInner::Component(component) => matches!(component, GenericValueComponent::Url(_)),
            ValueInner::List(list) => list
                .components
                .iter()
                .any(|component| matches!(component, GenericValueComponent::Url(_))),
            ValueInner::Universal(_) => false,
        }
    }

    fn serialization_types(&self) -> (TokenSerializationType, TokenSerializationType) {
        match &self.v {
            ValueInner::Component(component) => component.serialization_types(),
            ValueInner::Universal(_) => unreachable!(),
            ValueInner::List(list) => list
                .components
                .first()
                .map_or(Default::default(), |f| f.serialization_types()),
        }
    }

    /// Convert to an untyped variable value.
    pub fn to_variable_value(&self) -> ComputedPropertyValue {
        if let ValueInner::Universal(ref value) = self.v {
            return (**value).clone().with_attr_taint(self.attr_tainted);
        }
        let serialization_types = self.serialization_types();
        ComputedPropertyValue::new(
            self.to_css_string(),
            &self.url_data,
            serialization_types.0,
            serialization_types.1,
        )
        .with_attr_taint(self.attr_tainted)
    }
}

/// A specified registered custom property value.
#[derive(
    Animate, ToComputedValue, ToResolvedValue, ToCss, Clone, Debug, MallocSizeOf, PartialEq, ToShmem,
)]
pub enum ValueInner<Component> {
    /// A single specified component value whose syntax descriptor component did not have a
    /// multiplier.
    Component(Component),
    /// A specified value whose syntax descriptor was the universal syntax definition.
    #[animation(error)]
    Universal(#[ignore_malloc_size_of = "Arc"] Arc<ComputedPropertyValue>),
    /// A list of specified component values whose syntax descriptor component had a multiplier.
    List(#[animation(field_bound)] ComponentList<Component>),
}

/// Specified custom property value.
pub type SpecifiedValue = Value<SpecifiedValueComponent>;

/// Computed custom property value.
pub type ComputedValue = Value<ComputedValueComponent>;

impl SpecifiedValue {
    /// Compute a typed custom property with its original name available to random-key simplification.
    pub fn compute_with_property<'i, 't>(
        input: &mut CSSParser<'i, 't>,
        registration: &PropertyRegistrationData,
        url_data: &UrlExtraData,
        context: &computed::Context,
        allow: AllowComputationallyDependent,
        property: &str,
    ) -> Result<ComputedValue, ()> {
        Self::parse_with_property(input, &registration.syntax, url_data, allow, property)
            .map(|value| value.to_computed_value(context))
            .map_err(|_| ())
    }

    /// Convert a registered custom property to a Computed custom property value, given input and a
    /// property registration.
    pub fn compute<'i, 't>(
        input: &mut CSSParser<'i, 't>,
        registration: &PropertyRegistrationData,
        url_data: &UrlExtraData,
        context: &computed::Context,
        allow_computationally_dependent: AllowComputationallyDependent,
    ) -> Result<ComputedValue, ()> {
        debug_assert!(!registration.syntax.is_universal(), "Shouldn't be needed");
        let Ok(value) = Self::parse(
            input,
            &registration.syntax,
            url_data,
            allow_computationally_dependent,
        ) else {
            return Err(());
        };

        Ok(value.to_computed_value(context))
    }

    /// Parse and validate a registered custom property value according to its syntax descriptor,
    /// and check for computational independence.
    pub fn parse<'i, 't>(
        input: &mut CSSParser<'i, 't>,
        syntax: &Descriptor,
        url_data: &UrlExtraData,
        allow_computationally_dependent: AllowComputationallyDependent,
    ) -> Result<Self, StyleParseError<'i>> {
        Self::parse_internal(
            input,
            syntax,
            url_data,
            allow_computationally_dependent,
            None,
        )
    }

    /// Parse a typed custom property or function descriptor in its original declaration scope.
    pub fn parse_with_property<'i, 't>(
        input: &mut CSSParser<'i, 't>,
        syntax: &Descriptor,
        url_data: &UrlExtraData,
        allow: AllowComputationallyDependent,
        property: &str,
    ) -> Result<Self, StyleParseError<'i>> {
        Self::parse_internal(input, syntax, url_data, allow, Some(property))
    }

    fn parse_internal<'i, 't>(
        mut input: &mut CSSParser<'i, 't>,
        syntax: &Descriptor,
        url_data: &UrlExtraData,
        allow_computationally_dependent: AllowComputationallyDependent,
        property: Option<&str>,
    ) -> Result<Self, StyleParseError<'i>> {
        if syntax.is_universal() {
            let namespaces = crate::stylesheets::Namespaces::default();
            let parsed = ComputedPropertyValue::parse(&mut input, url_data, &namespaces)?;
            return Ok(SpecifiedValue {
                attr_tainted: parsed.is_attr_tainted(),
                v: ValueInner::Universal(Arc::new(parsed)),
                url_data: url_data.clone(),
            });
        }

        let mut values = SmallComponentVec::new();
        let mut multiplier = None;
        {
            let mut parser = Parser::new(syntax, &mut values, &mut multiplier);
            parser.parse(
                &mut input,
                url_data,
                allow_computationally_dependent,
                property,
            )?;
        }
        let v = if let Some(multiplier) = multiplier {
            ValueInner::List(ComponentList {
                multiplier,
                components: values.to_vec().into(),
            })
        } else {
            ValueInner::Component(values[0].clone())
        };
        Ok(Self {
            v,
            url_data: url_data.clone(),
            attr_tainted: false,
        })
    }
}

impl ComputedValueComponent {
    fn is_equivalent(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Color(computed::Color::Absolute(a)),
                Self::Color(computed::Color::Absolute(b)),
            ) => a.is_equivalent(b),
            _ => self == other,
        }
    }
}

/// Compare universal style-query values without declaration-boundary whitespace.
pub(crate) fn universal_values_match(a: Option<&str>, b: Option<&str>) -> bool {
    a.map(crate::custom_properties::trim_token_whitespace)
        == b.map(crate::custom_properties::trim_token_whitespace)
}

impl ComputedValue {
    /// Whether values match, ignoring universal boundary whitespace and equivalent colour representations.
    pub fn is_equivalent(&self, other: &Self) -> bool {
        match (&self.v, &other.v) {
            (ValueInner::Universal(a), ValueInner::Universal(b)) => {
                universal_values_match(Some(a.css_text()), Some(b.css_text()))
            },
            (ValueInner::Component(a), ValueInner::Component(b)) => a.is_equivalent(b),
            (ValueInner::List(a), ValueInner::List(b)) => {
                a.multiplier == b.multiplier
                    && a.components.len() == b.components.len()
                    && a.components
                        .iter()
                        .zip(b.components.iter())
                        .all(|(a, b)| a.is_equivalent(b))
            },
            (a, b) => a == b,
        }
    }

    fn to_declared_value(&self) -> properties::CustomDeclarationValue {
        if let ValueInner::Universal(_) = self.v {
            return properties::CustomDeclarationValue::Unparsed(Arc::new(
                self.to_variable_value(),
            ));
        }
        properties::CustomDeclarationValue::Parsed(Arc::new(ToComputedValue::from_computed_value(
            self,
        )))
    }

    /// Returns the contained variable value if it exists, otherwise `None`.
    pub fn as_universal(&self) -> Option<&Arc<ComputedPropertyValue>> {
        if let ValueInner::Universal(ref var) = self.v {
            Some(var)
        } else {
            None
        }
    }

    /// Parse this computed custom-property value as a specified CSS value
    /// using this value's stored URL data.
    pub fn parse_as<T>(&self, quirks_mode: QuirksMode, parsing_mode: ParsingMode) -> Result<T, ()>
    where
        T: Parse,
    {
        let variable_value = self.to_variable_value();
        let context = ParserContext::new(
            Origin::Author,
            &variable_value.url_data,
            None,
            parsing_mode,
            quirks_mode,
            Default::default(),
            None,
            None,
        );
        let mut input = cssparser::ParserInput::new(&variable_value.css);
        let mut input = CSSParser::new(&mut input);
        input
            .parse_entirely(|input| T::parse(&context, input))
            .map_err(|_| ())
    }
}

/// Whether the computed value parsing should allow computationaly dependent values like 3em or
/// var(-foo).
///
/// https://drafts.css-houdini.org/css-properties-values-api-1/#computationally-independent
pub enum AllowComputationallyDependent {
    /// Only computationally independent values are allowed.
    No,
    /// Computationally independent and dependent values are allowed.
    Yes,
}

type SmallComponentVec = SmallVec<[SpecifiedValueComponent; 1]>;

struct Parser<'a> {
    syntax: &'a Descriptor,
    output: &'a mut SmallComponentVec,
    output_multiplier: &'a mut Option<Multiplier>,
}

impl<'a> Parser<'a> {
    fn new(
        syntax: &'a Descriptor,
        output: &'a mut SmallComponentVec,
        output_multiplier: &'a mut Option<Multiplier>,
    ) -> Self {
        Self {
            syntax,
            output,
            output_multiplier,
        }
    }

    fn parse<'i, 't>(
        &mut self,
        input: &mut CSSParser<'i, 't>,
        url_data: &UrlExtraData,
        allow_computationally_dependent: AllowComputationallyDependent,
        property: Option<&str>,
    ) -> Result<(), StyleParseError<'i>> {
        use self::AllowComputationallyDependent::*;
        let parsing_mode = match allow_computationally_dependent {
            No => ParsingMode::DISALLOW_COMPUTATIONALLY_DEPENDENT,
            Yes => ParsingMode::DEFAULT,
        };
        let context = ParserContext::new(
            Origin::Author,
            url_data,
            Some(CssRuleType::Style),
            parsing_mode,
            QuirksMode::NoQuirks,
            /* namespaces = */ Default::default(),
            None,
            None,
        );
        let context = match property {
            Some(property) => context.for_random_declaration(property, input),
            None => context,
        };
        for component in self.syntax.components.iter() {
            let component = component.premultiplied();
            let result = input.try_parse(|input| {
                input.parse_entirely(|input| Self::parse_value(&context, input, &component))
            });
            let Ok(values) = result else { continue };
            self.output.extend(values);
            *self.output_multiplier = component.multiplier();
            break;
        }
        if self.output.is_empty() {
            return Err(input.new_error(BasicParseErrorKind::EndOfInput));
        }
        Ok(())
    }

    fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut CSSParser<'i, 't>,
        component: &SyntaxComponent,
    ) -> Result<SmallComponentVec, StyleParseError<'i>> {
        let mut values = SmallComponentVec::new();
        values.push(Self::parse_component_without_multiplier(
            context, input, component,
        )?);

        if let Some(multiplier) = component.multiplier() {
            loop {
                let result = Self::expect_multiplier(input, &multiplier);
                if Self::expect_multiplier_yielded_eof_error(&result) {
                    break;
                }
                result?;
                values.push(Self::parse_component_without_multiplier(
                    context, input, component,
                )?);
            }
        }
        Ok(values)
    }

    fn parse_component_without_multiplier<'i, 't>(
        context: &ParserContext,
        input: &mut CSSParser<'i, 't>,
        component: &SyntaxComponent,
    ) -> Result<SpecifiedValueComponent, StyleParseError<'i>> {
        let data_type = match component.name() {
            ComponentName::DataType(ty) => ty,
            ComponentName::Ident(ref name) => {
                let ident = CustomIdent::parse(input, &[])?;
                if ident != *name {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                return Ok(SpecifiedValueComponent::CustomIdent(ident));
            },
        };

        let value = match data_type {
            DataType::Length => {
                SpecifiedValueComponent::Length(specified::Length::parse(context, input)?)
            },
            DataType::Number => {
                SpecifiedValueComponent::Number(specified::Number::parse(context, input)?)
            },
            DataType::Percentage => {
                SpecifiedValueComponent::Percentage(specified::Percentage::parse(context, input)?)
            },
            DataType::LengthPercentage => SpecifiedValueComponent::LengthPercentage(
                specified::LengthPercentage::parse(context, input)?,
            ),
            DataType::Color => {
                SpecifiedValueComponent::Color(specified::Color::parse(context, input)?)
            },
            DataType::Image => {
                let image = specified::Image::parse(context, input)?;
                if matches!(image, specified::Image::None) {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                SpecifiedValueComponent::Image(image)
            },
            DataType::Url => {
                SpecifiedValueComponent::Url(specified::url::SpecifiedUrl::parse(context, input)?)
            },
            DataType::Integer => {
                SpecifiedValueComponent::Integer(specified::Integer::parse(context, input)?)
            },
            DataType::Angle => {
                SpecifiedValueComponent::Angle(specified::Angle::parse(context, input)?)
            },
            DataType::Time => {
                SpecifiedValueComponent::Time(specified::Time::parse(context, input)?)
            },
            DataType::Resolution => {
                SpecifiedValueComponent::Resolution(specified::Resolution::parse(context, input)?)
            },
            DataType::TransformFunction => SpecifiedValueComponent::TransformFunction(
                SingleTransformFunction(specified::TransformOperation::parse(context, input)?),
            ),
            DataType::CustomIdent => {
                let name = CustomIdent::parse(input, &[])?;
                SpecifiedValueComponent::CustomIdent(name)
            },
            DataType::TransformList => {
                let transform = specified::Transform::parse(context, input)?;
                if transform.0.is_empty() {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                SpecifiedValueComponent::TransformList(transform)
            },
            DataType::String => {
                let string = input.expect_string()?;
                SpecifiedValueComponent::String(string.as_ref().to_owned().into())
            },
        };
        Ok(value)
    }

    fn expect_multiplier_yielded_eof_error<'i>(result: &Result<(), StyleParseError<'i>>) -> bool {
        matches!(
            result,
            Err(StyleParseError {
                kind: ParseErrorKind::Basic(BasicParseErrorKind::EndOfInput),
                ..
            })
        )
    }

    fn expect_multiplier<'i, 't>(
        input: &mut CSSParser<'i, 't>,
        multiplier: &Multiplier,
    ) -> Result<(), StyleParseError<'i>> {
        match multiplier {
            Multiplier::Space => {
                input.expect_whitespace()?;
                if input.is_exhausted() {
                    // If there was trailing whitespace, do not interpret it as a multiplier
                    return Err(input.new_error(BasicParseErrorKind::EndOfInput));
                }
                Ok(())
            },
            Multiplier::Comma => Ok(input.expect_comma()?),
        }
    }
}

/// An animated value for custom property.
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct CustomAnimatedValue {
    /// The name of the custom property.
    pub(crate) name: crate::custom_properties::Name,
    /// The computed custom property value, or its guaranteed-invalid value when absent.
    pub(crate) value: Option<ComputedValue>,
}

impl Animate for CustomAnimatedValue {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self.name != other.name {
            return Err(());
        }
        let value = self
            .value
            .as_ref()
            .ok_or(())?
            .animate(other.value.as_ref().ok_or(())?, procedure)?;
        Ok(Self {
            name: self.name.clone(),
            value: Some(value),
        })
    }
}

impl CustomAnimatedValue {
    pub(crate) fn interpolable_with(&self, other: &Self) -> bool {
        self.name == other.name
            && self
                .value
                .as_ref()
                .zip(other.value.as_ref())
                .is_some_and(|(from, to)| {
                    from.v
                        .animate(&to.v, Procedure::Interpolate { progress: 0.5 })
                        .is_ok()
                })
    }

    pub(crate) fn from_computed(
        name: &crate::custom_properties::Name,
        value: Option<&ComputedValue>,
    ) -> Self {
        Self {
            name: name.clone(),
            value: value.cloned(),
        }
    }

    pub(crate) fn from_declaration(
        declaration: &properties::CustomDeclaration,
        context: &mut computed::Context,
        _initial: &properties::ComputedValues,
        attribute_tracker: &mut AttributeTracker,
    ) -> Option<Self> {
        let computed_value = match declaration.value {
            properties::CustomDeclarationValue::Unparsed(ref value) => {
                debug_assert!(
                    context.builder.stylist.is_some(),
                    "Need a Stylist to get property registration!"
                );
                let stylist = context.builder.stylist.unwrap();
                let registration = stylist.get_custom_property_registration(&declaration.name);
                if registration.syntax.is_universal() {
                    // A value that is invalid at computed-value time stays unparsed, as below.
                    let substituted = if value.has_references() {
                        crate::custom_properties::substitute(
                            value,
                            context.style().custom_properties(),
                            stylist,
                            context,
                            CascadeLevel::Animations,
                            attribute_tracker,
                        )
                        .map(|substituted| Arc::new(substituted.into_value(&value.url_data)))
                        .ok()
                    } else {
                        None
                    };
                    ComputedValue::universal(substituted.unwrap_or_else(|| Arc::clone(value)))
                } else {
                    // A value that is invalid at computed-value time stays unparsed: it animates
                    // discretely and does not compose.
                    Self::compute_registered(
                        &declaration.name,
                        value,
                        registration,
                        stylist,
                        context,
                        attribute_tracker,
                    )
                    .unwrap_or_else(|()| ComputedValue::universal(Arc::clone(value)))
                }
            },
            properties::CustomDeclarationValue::Parsed(ref v) => v.to_computed_value(context),
            // FIXME: This should be made to work to the extent possible like for non-custom
            // properties (using `initial` at least to handle unset / inherit).
            properties::CustomDeclarationValue::CSSWideKeyword(..) => return None,
        };
        Some(Self {
            name: declaration.name.clone(),
            value: Some(computed_value),
        })
    }

    fn compute_registered(
        name: &crate::custom_properties::Name,
        value: &ComputedPropertyValue,
        registration: &PropertyRegistrationData,
        stylist: &Stylist,
        context: &computed::Context,
        attribute_tracker: &mut AttributeTracker,
    ) -> Result<ComputedValue, ()> {
        let (css, attr_tainted) = if value.has_references() {
            let substituted = crate::custom_properties::substitute(
                value,
                context.style().custom_properties(),
                stylist,
                context,
                CascadeLevel::Animations,
                attribute_tracker,
            )?;
            (substituted.css, !substituted.attr_tainted.is_empty())
        } else {
            (Cow::Borrowed(value.css.as_str()), value.is_attr_tainted())
        };
        let mut input = cssparser::ParserInput::new(&css);
        SpecifiedValue::compute_with_property(
            &mut CSSParser::new(&mut input),
            registration,
            &value.url_data,
            context,
            AllowComputationallyDependent::Yes,
            &format!("--{}", name),
        )
        .map(|value| value.with_attr_taint(attr_tainted))
    }

    pub(crate) fn to_declaration(&self) -> properties::PropertyDeclaration {
        properties::PropertyDeclaration::Custom(properties::CustomDeclaration {
            name: self.name.clone(),
            value: self.value.as_ref().map_or(
                properties::CustomDeclarationValue::CSSWideKeyword(
                    properties::CSSWideKeyword::Initial,
                ),
                ComputedValue::to_declared_value,
            ),
        })
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::dom::DummyAttributeProvider;
    use crate::properties::{CustomDeclaration, CustomDeclarationValue, StyleBuilder};
    use crate::properties_and_values::registry::PropertyRegistration;
    use crate::properties_and_values::rule::{Inherits, PropertyRuleName};
    use crate::rule_cache::RuleCacheConditions;
    use crate::stylesheets::container_rule::ContainerSizeQuery;
    use crate::test_support::{test_device, with_computed_context};
    use crate::Atom;

    fn computed(syntax: &str, css: &str) -> Result<ComputedValue, ()> {
        let syntax = Descriptor::from_str(syntax, false).expect("the syntax must parse");
        let url_data = UrlExtraData::from(url::Url::parse("about:blank").unwrap());
        let mut input = cssparser::ParserInput::new(css);
        let value = CSSParser::new(&mut input)
            .parse_entirely(|input| {
                SpecifiedValue::parse(
                    input,
                    &syntax,
                    &url_data,
                    AllowComputationallyDependent::Yes,
                )
            })
            .map_err(|_| ())?;
        Ok(with_computed_context(|context| {
            value.to_computed_value(context)
        }))
    }

    fn animate(syntax: &str, from: &str, to: &str, procedure: Procedure) -> Result<String, ()> {
        let from = computed(syntax, from)?;
        Ok(from
            .animate(&computed(syntax, to)?, procedure)?
            .to_css_string())
    }

    #[test]
    fn registered_values_keep_attribute_taint_through_interpolation_and_discrete_sampling() {
        for (syntax, from, to) in [
            ("<length>", "1px", "3px"),
            ("<string>", "\"first\"", "\"second\""),
            ("*", "first", "second"),
        ] {
            for tainted_start in [true, false] {
                let from = computed(syntax, from)
                    .unwrap()
                    .with_attr_taint(tainted_start);
                let to = computed(syntax, to)
                    .unwrap()
                    .with_attr_taint(!tainted_start);
                for progress in [0.0, 0.25, 0.75, 1.0] {
                    let sampled = from
                        .animate(&to, Procedure::Interpolate { progress })
                        .unwrap();
                    assert!(
                        sampled.to_variable_value().is_attr_tainted(),
                        "{syntax}: {progress}"
                    );
                    let declared: SpecifiedValue = ToComputedValue::from_computed_value(&sampled);
                    assert!(with_computed_context(|context| {
                        let computed: ComputedValue = declared.to_computed_value(context);
                        computed.to_variable_value().is_attr_tainted()
                    }));
                    if syntax == "*" {
                        let properties::CustomDeclarationValue::Unparsed(declared) =
                            sampled.to_declared_value()
                        else {
                            panic!("universal animation values retain their token stream");
                        };
                        assert!(declared.is_attr_tainted());
                    }
                }
            }
        }
        let plain = computed("<string>", "\"same\"").unwrap();
        assert_ne!(plain, plain.clone().with_attr_taint(true));
        let plain = computed("*", "same").unwrap();
        let tainted = plain.clone().with_attr_taint(true);
        assert_ne!(plain, tainted);
        assert!(plain.is_equivalent(&tainted));
        let tainted = computed("<number>", "1").unwrap().with_attr_taint(true);
        let plain = computed("<number>", "2").unwrap();
        for procedure in [Procedure::Add, Procedure::Accumulate { count: 2 }] {
            assert!(tainted
                .animate(&plain, procedure)
                .unwrap()
                .to_variable_value()
                .is_attr_tainted());
        }
    }

    #[test]
    fn registered_colour_interpolation_keeps_endpoint_representation_and_combines_taint() {
        let from = computed("<color>", "color(srgb 1 0.2 0.3)")
            .unwrap()
            .with_attr_taint(true);
        let to = computed("<color>", "color(srgb 0.4 0.5 1)").unwrap();
        for (progress, expected) in [(0.0, &from), (1.0, &to)] {
            let sampled = from
                .animate(&to, Procedure::Interpolate { progress })
                .unwrap();
            assert_eq!(sampled.to_css_string(), expected.to_css_string());
            assert!(sampled.to_variable_value().is_attr_tainted());
        }
    }

    #[test]
    fn custom_property_transition_interpolability_excludes_discrete_values() {
        let name = Atom::from("value");
        let number = computed("<number>", "1").unwrap();
        let number = CustomAnimatedValue::from_computed(&name, Some(&number));
        assert!(number.interpolable_with(&number));
        let string = computed("<string>", "\"value\"").unwrap();
        let string = CustomAnimatedValue::from_computed(&name, Some(&string));
        assert!(!string.interpolable_with(&string));
        let invalid = CustomAnimatedValue::from_computed(&name, None);
        assert!(!invalid.interpolable_with(&number));
    }

    #[test]
    fn container_relative_lengths_are_not_computationally_independent() {
        let syntax = Descriptor::from_str("<length>", false).expect("the syntax must parse");
        let url_data = UrlExtraData::from(url::Url::parse("about:blank").unwrap());
        for (css, independent) in [
            ("1px", true),
            ("1vw", true),
            ("1cqw", false),
            ("1cqmax", false),
        ] {
            let mut input = cssparser::ParserInput::new(css);
            let parsed = CSSParser::new(&mut input).parse_entirely(|input| {
                SpecifiedValue::parse(input, &syntax, &url_data, AllowComputationallyDependent::No)
            });
            assert_eq!(parsed.is_ok(), independent, "{css}");
        }
    }

    #[test]
    fn transform_function_is_one_function() {
        assert!(computed("<transform-function>", "translateX(1px) scale(2)").is_err());
        assert!(computed("<transform-function>", "none").is_err());
    }

    #[test]
    fn transform_function_adds_by_accumulation() {
        assert_eq!(
            animate(
                "<transform-function>#",
                "translateX(10px), scale(2)",
                "translateX(20px), scale(3)",
                Procedure::Add,
            ),
            Ok("translateX(30px), scale(4)".to_owned()),
        );
    }

    #[test]
    fn transform_function_does_not_add_to_transform_list() {
        assert_eq!(
            animate(
                "<transform-function> | <transform-list>",
                "translateX(100px) scale(2)",
                "translateX(200px)",
                Procedure::Add,
            ),
            Err(()),
        );
    }

    #[test]
    fn transform_list_adds_by_concatenation() {
        assert_eq!(
            animate(
                "<transform-list>",
                "translateX(100px)",
                "scale(2)",
                Procedure::Add
            ),
            Ok("translateX(100px) scale(2)".to_owned()),
        );
    }

    #[test]
    fn transform_function_list_interpolates_as_transform_list() {
        assert_eq!(
            animate(
                "<transform-function>+",
                "translateX(100px) scale(2)",
                "translateX(200px)",
                Procedure::Interpolate { progress: 0.5 },
            ),
            Ok("translateX(150px) scale(1.5)".to_owned()),
        );
    }

    fn keyframe_value(syntax: &str, css: &str) -> ComputedValue {
        let name = Atom::from("x");
        let url_data = UrlExtraData::from(url::Url::parse("about:blank").unwrap());
        let mut stylist = Stylist::new(test_device(), QuirksMode::NoQuirks);
        stylist
            .custom_property_script_registry_mut()
            .register(PropertyRegistration {
                name: PropertyRuleName(name.clone()),
                data: PropertyRegistrationData {
                    syntax: Descriptor::from_str(syntax, false).expect("the syntax must parse"),
                    inherits: Inherits::False,
                    initial_value: None,
                },
                url_data: url_data.clone(),
                source_location: cssparser::SourceLocation { line: 0, column: 0 },
            });
        let mut input = cssparser::ParserInput::new(css);
        let value = CSSParser::new(&mut input)
            .parse_entirely(|input| {
                ComputedPropertyValue::parse(input, &url_data, &Default::default())
            })
            .expect("the declaration value must parse");
        let declaration = CustomDeclaration {
            name,
            value: CustomDeclarationValue::Unparsed(Arc::new(value)),
        };
        let initial_values = stylist.device().default_computed_values();
        let mut conditions = RuleCacheConditions::default();
        let mut context = computed::Context::new(
            StyleBuilder::for_derived_style(stylist.device(), Some(&stylist), initial_values, None),
            QuirksMode::NoQuirks,
            &mut conditions,
            ContainerSizeQuery::none(),
        );
        CustomAnimatedValue::from_declaration(
            &declaration,
            &mut context,
            initial_values,
            &mut AttributeTracker::new(&DummyAttributeProvider),
        )
        .expect("an unparsed declaration has an animated value")
        .value
        .expect("the declaration retains its computed or unparsed value")
    }

    #[test]
    fn keyframe_value_substitutes_references() {
        let value = keyframe_value("<length>", "var(--missing, 1px)");
        assert!(value.as_universal().is_none());
        assert_eq!(value.to_css_string(), "1px");
    }

    #[test]
    fn keyframe_value_invalid_at_computed_value_time_stays_unparsed() {
        let value = keyframe_value("<length>", "var(--missing, invalid)");
        assert!(value.as_universal().is_some());
    }

    #[test]
    fn untyped_keyframe_value_substitutes_references() {
        let value = keyframe_value("*", "a var(--missing, 1px)");
        assert_eq!(value.to_css_string(), "a 1px");
    }
}
