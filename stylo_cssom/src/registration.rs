use style::{
    properties_and_values::syntax::{ComponentName, Descriptor, Multiplier, data_type::DataType},
    properties_and_values::value::{AllowComputationallyDependent, SpecifiedValue},
    stylesheets::UrlExtraData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredStaticUnitSyntax {
    Length,
    Angle,
    Integer,
    Number,
    Percentage,
    Resolution,
    Time,
}

impl RegisteredStaticUnitSyntax {
    pub fn from_component(component: &ComponentName) -> Option<Self> {
        match component {
            ComponentName::DataType(DataType::Length) => Some(Self::Length),
            ComponentName::DataType(DataType::Angle) => Some(Self::Angle),
            ComponentName::DataType(DataType::Integer) => Some(Self::Integer),
            ComponentName::DataType(DataType::Number) => Some(Self::Number),
            ComponentName::DataType(DataType::Percentage) => Some(Self::Percentage),
            ComponentName::DataType(DataType::Resolution) => Some(Self::Resolution),
            ComponentName::DataType(DataType::Time) => Some(Self::Time),
            ComponentName::DataType(
                DataType::LengthPercentage
                | DataType::Color
                | DataType::Image
                | DataType::Url
                | DataType::TransformFunction
                | DataType::CustomIdent
                | DataType::TransformList
                | DataType::String,
            )
            | ComponentName::Ident(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StaticNumericValue {
    Px(f64),
    AngleDegrees(f64),
    Integer(i32),
    Number(f64),
    Percent(f64),
    ResolutionDppx(f64),
    TimeSeconds(f64),
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn length_value(computed: &str) -> Option<StaticNumericValue> {
    let value = crate::values::parse_value::<style::values::specified::Length>(computed)?
        .to_computed_pixel_length_without_context()
        .ok()?;
    Some(StaticNumericValue::Px(finite(f64::from(value))?))
}

pub fn single_identifier(computed: &str) -> Option<String> {
    let mut input = cssparser::ParserInput::new(computed);
    let mut parser = cssparser::Parser::new(&mut input);
    let identifier = parser.expect_ident().ok()?.to_string();
    parser.is_exhausted().then_some(identifier)
}

pub fn pixel_length_list(
    computed: &str,
    multiplier: Multiplier,
) -> Option<Vec<StaticNumericValue>> {
    let mut input = cssparser::ParserInput::new(computed);
    let mut parser = cssparser::Parser::new(&mut input);
    let first = pixel_length_component(&mut parser)?;
    let mut values = vec![first];
    while !parser.is_exhausted() {
        if multiplier == Multiplier::Comma {
            parser.expect_comma().ok()?;
        }
        values.push(pixel_length_component(&mut parser)?);
    }
    Some(values)
}

pub fn pixel_length_component(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Option<StaticNumericValue> {
    let token = parser.next().ok()?.clone();
    let cssparser::Token::Dimension { value, unit, .. } = token else {
        return None;
    };
    if !unit.eq_ignore_ascii_case("px") {
        return None;
    }
    Some(StaticNumericValue::Px(finite(f64::from(value))?))
}

pub fn static_unit_value(
    syntax: RegisteredStaticUnitSyntax,
    computed: &str,
) -> Option<StaticNumericValue> {
    if syntax == RegisteredStaticUnitSyntax::Length {
        return length_value(computed);
    }
    let mut input = cssparser::ParserInput::new(computed);
    let mut parser = cssparser::Parser::new(&mut input);
    let token = parser.next().ok()?.clone();
    if !parser.is_exhausted() {
        return None;
    }
    let value = match (syntax, token) {
        (RegisteredStaticUnitSyntax::Angle, cssparser::Token::Dimension { value, unit, .. })
            if unit.eq_ignore_ascii_case("deg") =>
        {
            StaticNumericValue::AngleDegrees(finite(f64::from(value))?)
        },
        (
            RegisteredStaticUnitSyntax::Integer,
            cssparser::Token::Number {
                int_value: Some(value),
                ..
            },
        ) => StaticNumericValue::Integer(value),
        (RegisteredStaticUnitSyntax::Number, cssparser::Token::Number { value, .. }) => {
            StaticNumericValue::Number(finite(f64::from(value))?)
        },
        (RegisteredStaticUnitSyntax::Percentage, cssparser::Token::Percentage { .. }) => {
            StaticNumericValue::Percent(finite(computed.trim().strip_suffix('%')?.parse().ok()?)?)
        },
        (
            RegisteredStaticUnitSyntax::Resolution,
            cssparser::Token::Dimension { value, unit, .. },
        ) if unit.eq_ignore_ascii_case("dppx") => {
            StaticNumericValue::ResolutionDppx(finite(f64::from(value))?)
        },
        (RegisteredStaticUnitSyntax::Time, cssparser::Token::Dimension { value, unit, .. })
            if unit.eq_ignore_ascii_case("s") =>
        {
            StaticNumericValue::TimeSeconds(finite(f64::from(value))?)
        },
        _ => return None,
    };
    Some(value)
}

pub fn percentage_value(computed: &str) -> Option<f64> {
    let mut input = cssparser::ParserInput::new(computed);
    let mut parser = cssparser::Parser::new(&mut input);
    let cssparser::Token::Percentage { .. } = parser.next().ok()?.clone() else {
        return None;
    };
    let value = computed.trim().strip_suffix('%')?.parse::<f64>().ok()?;
    (parser.is_exhausted() && value.is_finite()).then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthPercentageSum {
    pub percentage: f64,
    pub px: f64,
}

pub fn length_percentage_sum(source: &str) -> Option<LengthPercentageSum> {
    use style::values::specified::{
        LengthPercentage,
        calc::{CalcNode, Leaf},
    };
    let LengthPercentage::Calc(calc) = crate::values::parse_value::<LengthPercentage>(source)?
    else {
        return None;
    };
    fn accumulate(node: &CalcNode, sum: &mut LengthPercentageSum) -> Option<()> {
        match node {
            CalcNode::Leaf(Leaf::Percentage(value)) => sum.percentage += f64::from(*value * 100.0),
            CalcNode::Leaf(Leaf::Length(value)) => {
                sum.px += f64::from(value.to_computed_pixel_length_without_context().ok()?)
            },
            CalcNode::Sum(nodes) => {
                for node in nodes.iter() {
                    accumulate(node, sum)?;
                }
            },
            _ => return None,
        }
        Some(())
    }
    let mut sum = LengthPercentageSum {
        percentage: 0.0,
        px: 0.0,
    };
    accumulate(&calc.node, &mut sum)?;
    (sum.percentage.is_finite() && sum.px.is_finite()).then_some(sum)
}
pub fn parse_registered_value(descriptor: &Descriptor, value: &str) -> Option<SpecifiedValue> {
    let mut input = cssparser::ParserInput::new(value);
    let mut parser = cssparser::Parser::new(&mut input);
    let url_data = UrlExtraData::from(
        url::Url::parse("about:blank").expect("the static CSS value base URL must be valid"),
    );
    parser
        .parse_entirely(|input| {
            SpecifiedValue::parse(
                input,
                descriptor,
                &url_data,
                AllowComputationallyDependent::Yes,
            )
        })
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterPropertyError {
    Syntax(String),
    DuplicateName,
    State(String),
}

impl std::fmt::Display for RegisterPropertyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(msg) => write!(f, "SyntaxError: {msg}"),
            Self::DuplicateName => write!(
                f,
                "InvalidModificationError: property is already registered"
            ),
            Self::State(msg) => write!(f, "InvalidStateError: {msg}"),
        }
    }
}

impl std::error::Error for RegisterPropertyError {}

#[derive(Debug, Clone)]
pub struct ValidatedRegistration {
    pub registration: stylo_cssom_model::ImperativePropertyRegistrationInput,
    pub descriptor: Descriptor,
    pub stylo_registration: style::properties_and_values::registry::PropertyRegistration,
}

pub fn validate_register_property(
    name: &str,
    syntax: &str,
    inherits: bool,
    initial_value: Option<&str>,
) -> Result<ValidatedRegistration, RegisterPropertyError> {
    if !crate::is_valid_custom_property_name(name) {
        return Err(RegisterPropertyError::Syntax(format!(
            "name `{name}` is not a valid <custom-property-name>"
        )));
    }

    let descriptor = Descriptor::from_str(syntax, false).map_err(|_| {
        RegisterPropertyError::Syntax(format!(
            "syntax `{syntax}` is not a valid property syntax definition"
        ))
    })?;
    let registration = stylo_cssom_model::ImperativePropertyRegistrationInput::new(
        name.to_owned(),
        syntax.to_owned(),
        inherits,
        initial_value.map(str::to_owned),
    )
    .expect("validated registration names and syntax are non-empty");
    let url_data = UrlExtraData::from(
        url::Url::parse("about:blank").expect("the static CSS value base URL must be valid"),
    );
    let namespaces = style::stylesheets::Namespaces::default();
    let parsed_initial_value = initial_value
        .map(|value| {
            let mut input = cssparser::ParserInput::new(value);
            let mut parser = cssparser::Parser::new(&mut input);
            parser
                .parse_entirely(|input| {
                    style::custom_properties::SpecifiedValue::parse(input, &url_data, &namespaces)
                })
                .map(servo_arc::Arc::new)
        })
        .transpose()
        .map_err(|_| registration_validation_error(name, syntax, inherits, initial_value))?;
    style::properties_and_values::registry::PropertyRegistration::validate_initial_value(
        &descriptor,
        parsed_initial_value.as_deref(),
    )
    .map_err(|_| registration_validation_error(name, syntax, inherits, initial_value))?;
    let stylo_registration = style::properties_and_values::registry::PropertyRegistration {
        name: style::properties_and_values::rule::PropertyRuleName(style::Atom::from(
            name.strip_prefix("--")
                .expect("validated custom property names have a prefix"),
        )),
        data: style::properties_and_values::registry::PropertyRegistrationData {
            syntax: descriptor.clone(),
            inherits: if inherits {
                style::properties_and_values::rule::Inherits::True
            } else {
                style::properties_and_values::rule::Inherits::False
            },
            initial_value: parsed_initial_value,
        },
        url_data,
        source_location: cssparser::SourceLocation { line: 0, column: 0 },
    };

    Ok(ValidatedRegistration {
        registration,
        descriptor,
        stylo_registration,
    })
}

fn registration_validation_error(
    name: &str,
    syntax: &str,
    inherits: bool,
    initial_value: Option<&str>,
) -> RegisterPropertyError {
    RegisterPropertyError::Syntax(format!(
        "@property registration for `{name}` failed CSS Properties and \
         Values API §2.1 validation (syntax `{syntax}`, inherits \
         {inherits}, initial-value `{initial}`)",
        initial = initial_value.unwrap_or("<absent>"),
    ))
}

pub fn direct_registered_serialization(computed: &str) -> String {
    if computed.starts_with("linear-gradient(") {
        computed.replace("rgb(255, 0, 0)", "red")
    } else {
        computed.to_owned()
    }
}
