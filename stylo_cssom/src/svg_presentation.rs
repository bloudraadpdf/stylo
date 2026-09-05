use style::properties::{ComputedValues, LonghandId, PropertyDeclarationId};

pub fn svg_style_value_is_none(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("none")
}

#[derive(Clone, Copy)]
struct SvgLonghandNormaliser {
    longhand: LonghandId,
    normalise: fn(&str, bool, &dyn Fn(&str) -> Option<String>) -> Option<String>,
}

const SVG_LONGHAND_NORMALISERS: &[SvgLonghandNormaliser] = &[
    SvgLonghandNormaliser {
        longhand: LonghandId::ClipPath,
        normalise: |v, _, resolve_url| normalise_svg_url_reference(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::Filter,
        normalise: |v, _, resolve_url| normalise_svg_filter_list(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::Fill,
        normalise: |v, _, resolve_url| normalise_svg_paint_value(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::Stroke,
        normalise: |v, _, resolve_url| normalise_svg_paint_value(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::MarkerStart,
        normalise: |v, _, resolve_url| normalise_svg_url_reference(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::MarkerMid,
        normalise: |v, _, resolve_url| normalise_svg_url_reference(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::MarkerEnd,
        normalise: |v, _, resolve_url| normalise_svg_url_reference(v, resolve_url),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::Transform,
        normalise: |v, _, _| normalise_svg_transform_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::TransformOrigin,
        normalise: |v, has_transform, _| normalise_svg_transform_origin_value(v, has_transform),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::TransformBox,
        normalise: |v, has_transform, _| normalise_svg_transform_box_value(v, has_transform),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::LetterSpacing,
        normalise: |v, _, _| normalise_svg_spacing_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::WordSpacing,
        normalise: |v, _, _| normalise_svg_spacing_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::FontStyle,
        normalise: |v, _, _| normalise_svg_font_style_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::FontStretch,
        normalise: |v, _, _| normalise_svg_font_stretch_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::FillOpacity,
        normalise: |v, _, _| normalise_svg_opacity_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::StrokeOpacity,
        normalise: |v, _, _| normalise_svg_opacity_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::WritingMode,
        normalise: |v, _, _| normalise_svg_writing_mode_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::ColorInterpolation,
        normalise: |v, _, _| normalise_svg_color_interpolation_value(v),
    },
    SvgLonghandNormaliser {
        longhand: LonghandId::ColorInterpolationFilters,
        normalise: |v, _, _| normalise_svg_color_interpolation_value(v),
    },
];

pub fn normalise_svg_presentation_declaration(
    longhand: LonghandId,
    css_name: &str,
    value: &str,
    has_effective_transform: bool,
    resolve_url: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(entry) = SVG_LONGHAND_NORMALISERS
        .iter()
        .find(|entry| entry.longhand == longhand)
    {
        return (entry.normalise)(trimmed, has_effective_transform, resolve_url);
    }

    normalise_svg_presentation_value(css_name, trimmed)
}

fn normalise_svg_url_reference(
    trimmed: &str,
    resolve_url: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if svg_style_value_is_none(trimmed) {
        return None;
    }
    if !svg_url_reference_value_supported(trimmed) {
        return None;
    }
    Some(strip_document_base_from_url_value(trimmed, resolve_url))
}

fn normalise_svg_filter_list(
    trimmed: &str,
    resolve_url: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if svg_style_value_is_none(trimmed) {
        return None;
    }
    crate::values::filter_component_ranges(trimmed)?
        .into_iter()
        .map(|(range, is_url)| {
            let component = trimmed.get(range)?.trim();
            if is_url {
                svg_url_reference_value_supported(component)
                    .then(|| strip_document_base_from_url_value(component, resolve_url))
            } else {
                Some(component.to_owned())
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join(" "))
}

fn normalise_svg_paint_value(
    trimmed: &str,
    resolve_url: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if trimmed.is_empty() {
        return None;
    }

    let lower_head = trimmed.get(..4).map(str::to_ascii_lowercase);
    if lower_head.as_deref() != Some("url(") {
        return Some(trimmed.to_string());
    }
    let close = trimmed.find(')')?;
    let head = &trimmed[..=close];
    let tail = trimmed[close + 1..].trim_start();
    let stripped_head = strip_document_base_from_url_value(head, resolve_url);
    if tail.is_empty() {
        Some(stripped_head)
    } else {
        Some(format!("{stripped_head} {tail}"))
    }
}

fn strip_document_base_from_url_value(
    value: &str,
    resolve_url: &dyn Fn(&str) -> Option<String>,
) -> String {
    let trimmed = value.trim();
    let inner = match trimmed
        .strip_prefix("url(")
        .or_else(|| trimmed.strip_prefix("URL("))
        .and_then(|v| v.strip_suffix(')'))
    {
        Some(inner) => inner.trim(),

        None => return value.to_string(),
    };
    let unquoted = strip_url_quotes(inner);
    if let Some(fragment) = resolve_url(unquoted) {
        return format!("url(#{fragment})");
    }

    format!("url({unquoted})")
}

fn strip_url_quotes(inner: &str) -> &str {
    let bytes = inner.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return &inner[1..inner.len() - 1];
        }
    }
    inner
}

fn normalise_svg_transform_value(trimmed: &str) -> Option<String> {
    if svg_style_value_is_none(trimmed) || trimmed.is_empty() {
        return None;
    }
    let functions = parse_svg_transform_function_list(trimmed)?;
    let mut converted = Vec::with_capacity(functions.len());
    for func in functions {
        converted.push(convert_svg_transform_function(func)?);
    }
    Some(converted.join(" "))
}

#[derive(Debug, Clone)]
struct SvgTransformFunction<'a> {
    name: String,
    args: Vec<&'a str>,
}

fn parse_svg_transform_function_list(value: &str) -> Option<Vec<SvgTransformFunction<'_>>> {
    use cssparser::{Parser, ParserInput, Token};
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let mut functions = Vec::new();
    while !parser.is_exhausted() {
        let Token::Function(name) = parser.next().ok()?.clone() else {
            return None;
        };
        let args = parser
            .parse_nested_block(|input| {
                let mut args = Vec::new();
                while !input.is_exhausted() {
                    input.skip_whitespace();
                    let start = input.position();
                    let token = input.next()?.clone();
                    if matches!(token, Token::Comma) {
                        return Err(input.new_custom_error(()));
                    }
                    if matches!(token, Token::Function(_) | Token::ParenthesisBlock) {
                        input.parse_nested_block(
                            |input| -> Result<(), cssparser::ParseError<'_, ()>> {
                                while input.next_including_whitespace_and_comments().is_ok() {}
                                Ok(())
                            },
                        )?;
                    }
                    args.push(input.slice_from(start).trim());
                    if input.try_parse(Parser::expect_comma).is_ok() && input.is_exhausted() {
                        return Err(input.new_custom_error(()));
                    }
                }
                Ok::<_, cssparser::ParseError<'_, ()>>(args)
            })
            .ok()?;
        functions.push(SvgTransformFunction {
            name: name.to_string(),
            args,
        });
        if parser.try_parse(Parser::expect_comma).is_ok() && parser.is_exhausted() {
            return None;
        }
    }
    (!functions.is_empty()).then_some(functions)
}

#[allow(clippy::needless_pass_by_value)]
fn convert_svg_transform_function(func: SvgTransformFunction<'_>) -> Option<String> {
    let kind = SvgTransformKind::from_name(&func.name)?;
    match kind {
        SvgTransformKind::AngleOnly { canonical } => {
            convert_angle_only_function(canonical, &func.args)
        },
        SvgTransformKind::RotateWithOptionalCentre => convert_rotate_function(&func.args),
        SvgTransformKind::Passthrough { canonical } => {
            Some(format!("{canonical}({})", func.args.join(", ")))
        },
    }
}

#[derive(Debug, Clone, Copy)]
enum SvgTransformKind {
    AngleOnly { canonical: &'static str },

    RotateWithOptionalCentre,

    Passthrough { canonical: &'static str },
}

impl SvgTransformKind {
    fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("rotate") {
            Some(SvgTransformKind::RotateWithOptionalCentre)
        } else if name.eq_ignore_ascii_case("skewX") {
            Some(SvgTransformKind::AngleOnly { canonical: "skewX" })
        } else if name.eq_ignore_ascii_case("skewY") {
            Some(SvgTransformKind::AngleOnly { canonical: "skewY" })
        } else if name.eq_ignore_ascii_case("translate") {
            Some(SvgTransformKind::Passthrough {
                canonical: "translate",
            })
        } else if name.eq_ignore_ascii_case("translateX") {
            Some(SvgTransformKind::Passthrough {
                canonical: "translateX",
            })
        } else if name.eq_ignore_ascii_case("translateY") {
            Some(SvgTransformKind::Passthrough {
                canonical: "translateY",
            })
        } else if name.eq_ignore_ascii_case("scale") {
            Some(SvgTransformKind::Passthrough { canonical: "scale" })
        } else if name.eq_ignore_ascii_case("scaleX") {
            Some(SvgTransformKind::Passthrough {
                canonical: "scaleX",
            })
        } else if name.eq_ignore_ascii_case("scaleY") {
            Some(SvgTransformKind::Passthrough {
                canonical: "scaleY",
            })
        } else if name.eq_ignore_ascii_case("matrix") {
            Some(SvgTransformKind::Passthrough {
                canonical: "matrix",
            })
        } else {
            None
        }
    }
}

fn convert_angle_only_function(name: &str, args: &[&str]) -> Option<String> {
    if args.len() != 1 {
        return None;
    }
    let degrees = parse_svg_transform_angle(args[0])?;
    Some(format!("{name}({})", format_svg_number(degrees)))
}

fn convert_rotate_function(args: &[&str]) -> Option<String> {
    match args.len() {
        1 => {
            let degrees = parse_svg_transform_angle(args[0])?;
            Some(format!("rotate({})", format_svg_number(degrees)))
        },
        3 => {
            let degrees = parse_svg_transform_angle(args[0])?;
            Some(format!(
                "rotate({}, {}, {})",
                format_svg_number(degrees),
                args[1],
                args[2]
            ))
        },
        _ => None,
    }
}

fn parse_svg_transform_angle(arg: &str) -> Option<f64> {
    let trimmed = arg.trim();
    if let Some(n) = crate::values::parse_value::<style::values::specified::Number>(trimmed)
        .map(|value| value.get())
    {
        return Some(f64::from(n));
    }
    let degrees = crate::values::parse_value::<style::values::specified::Angle>(trimmed)?.degrees();
    Some(f64::from(degrees))
}

fn format_svg_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let formatted = format!("{value:.6}");
    if !formatted.contains('.') {
        return formatted;
    }
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');

    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalise_svg_transform_origin_value(
    trimmed: &str,
    has_effective_transform: bool,
) -> Option<String> {
    has_effective_transform.then(|| trimmed.to_string())
}

fn normalise_svg_transform_box_value(
    trimmed: &str,
    has_effective_transform: bool,
) -> Option<String> {
    has_effective_transform.then(|| trimmed.to_string())
}

fn normalise_svg_spacing_value(trimmed: &str) -> Option<String> {
    (!trimmed.eq_ignore_ascii_case("normal")).then(|| trimmed.to_string())
}

fn normalise_svg_opacity_value(trimmed: &str) -> Option<String> {
    if trimmed == "1" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalise_svg_writing_mode_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("horizontal-tb")
        || trimmed.eq_ignore_ascii_case("lr")
        || trimmed.eq_ignore_ascii_case("lr-tb")
        || trimmed.eq_ignore_ascii_case("rl")
        || trimmed.eq_ignore_ascii_case("rl-tb")
    {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("vertical-rl")
        || trimmed.eq_ignore_ascii_case("vertical-lr")
        || trimmed.eq_ignore_ascii_case("sideways-rl")
        || trimmed.eq_ignore_ascii_case("sideways-lr")
        || trimmed.eq_ignore_ascii_case("tb")
        || trimmed.eq_ignore_ascii_case("tb-rl")
    {
        return Some("vertical-rl".to_string());
    }
    None
}

fn normalise_svg_presentation_value(property: &str, value: &str) -> Option<String> {
    match property {
        "marker-start" | "marker-mid" | "marker-end" => {
            svg_url_reference_value_supported(value).then(|| value.to_string())
        },
        _ => Some(value.to_string()),
    }
}

pub fn computed_svg_overflow_presentation_value(computed: &ComputedValues) -> Option<String> {
    let overflow_x =
        computed.computed_value_to_string(PropertyDeclarationId::Longhand(LonghandId::OverflowX));
    let overflow_y =
        computed.computed_value_to_string(PropertyDeclarationId::Longhand(LonghandId::OverflowY));

    if overflow_x.is_empty() || overflow_x != overflow_y {
        return None;
    }

    Some(overflow_x)
}

fn svg_url_reference_value_supported(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.eq_ignore_ascii_case("none")
        || (trimmed.len() >= 5
            && trimmed[..4].eq_ignore_ascii_case("url(")
            && trimmed.ends_with(')'))
}

fn normalise_svg_color_interpolation_value(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some("auto".into()),
        "linearrgb" => Some("linearRGB".into()),
        "srgb" => Some("sRGB".into()),
        _ => None,
    }
}

#[must_use]
pub fn restore_svg_case_sensitive_keyword_casing(property: &str, value: &str) -> Option<String> {
    if property.eq_ignore_ascii_case("color-interpolation")
        || property.eq_ignore_ascii_case("color-interpolation-filters")
    {
        return normalise_svg_color_interpolation_value(value);
    }
    None
}

fn normalise_svg_font_style_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("normal") || trimmed.eq_ignore_ascii_case("italic") {
        Some(trimmed.to_ascii_lowercase())
    } else if trimmed.eq_ignore_ascii_case("oblique")
        || trimmed
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("oblique "))
    {
        Some("oblique".to_string())
    } else {
        None
    }
}

fn normalise_svg_font_stretch_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "ultra-condensed" | "extra-condensed" | "condensed" | "semi-condensed" | "normal"
        | "semi-expanded" | "expanded" | "extra-expanded" | "ultra-expanded" | "narrower"
        | "wider" => {
            return Some(trimmed.to_ascii_lowercase());
        },
        _ => {},
    }

    let percent =
        crate::values::parse_value::<style::values::specified::Percentage>(trimmed)?.get() * 100.0;
    let keyword = [
        (50.0, "ultra-condensed"),
        (62.5, "extra-condensed"),
        (75.0, "condensed"),
        (87.5, "semi-condensed"),
        (100.0, "normal"),
        (112.5, "semi-expanded"),
        (125.0, "expanded"),
        (150.0, "extra-expanded"),
        (200.0, "ultra-expanded"),
    ]
    .into_iter()
    .find_map(|(expected, keyword)| ((percent - expected).abs() <= 0.05).then_some(keyword))?;
    Some(keyword.to_string())
}

pub fn standalone_length_value_supported(value: &str) -> bool {
    crate::values::parse_value::<style::values::specified::LengthPercentage>(value).is_some()
        || crate::values::parse_value::<style::values::specified::Number>(value).is_some()
}

pub fn standalone_declaration(
    property: &str,
    value: &str,
    important: bool,
    root_points: Option<[f32; 2]>,
) -> Option<stylo_cssom_model::RuleDeclaration> {
    let property = property.trim().to_ascii_lowercase();
    let value = value.trim();
    if value.is_empty() || (property == "letter-spacing" && value.eq_ignore_ascii_case("normal")) {
        return None;
    }
    let value = if let Some([width, height]) = root_points
        && matches!(property.as_str(), "width" | "height")
        && !standalone_length_value_supported(value)
    {
        let resolved = if property == "width" { width } else { height };
        if resolved == 0.0 {
            return None;
        }
        format!("{}pt", format_svg_number(f64::from(resolved)))
    } else {
        restore_svg_case_sensitive_keyword_casing(&property, value)
            .unwrap_or_else(|| value.to_owned())
    };
    Some(stylo_cssom_model::RuleDeclaration::new(property, value).with_importance(important))
}

fn user_length(value: &str) -> Option<f32> {
    crate::values::parse_value::<style::values::specified::Length>(value)
        .and_then(|length| length.to_computed_pixel_length_without_context().ok())
        .or_else(|| {
            crate::values::parse_value::<style::values::specified::Number>(value)
                .map(|number| number.get())
        })
}

fn translation(value: &str, extent: f32) -> Option<f32> {
    crate::values::parse_value::<style::values::specified::Percentage>(value)
        .map(|percentage| extent * percentage.get())
        .or_else(|| user_length(value))
}

pub fn resolve_transform_for_reference_box(
    source: &str,
    width: f32,
    height: f32,
) -> Option<String> {
    let functions = parse_svg_transform_function_list(source)?;
    functions
        .into_iter()
        .map(|function| {
            let number = |value: f32| format_svg_number(f64::from(value));
            let text = match (
                function.name.to_ascii_lowercase().as_str(),
                function.args.as_slice(),
            ) {
                ("translate", [x]) => format!("translate({} 0)", number(translation(x, width)?)),
                ("translate", [x, y]) => format!(
                    "translate({} {})",
                    number(translation(x, width)?),
                    number(translation(y, height)?)
                ),
                ("translatex", [x]) => format!("translate({} 0)", number(translation(x, width)?)),
                ("translatey", [y]) => format!("translate(0 {})", number(translation(y, height)?)),
                ("translate" | "translatex" | "translatey", _) => return None,
                _ => format!("{}({})", function.name, function.args.join(", ")),
            };
            Some(text)
        })
        .collect::<Option<Vec<_>>>()
        .map(|functions| functions.join(" "))
}

pub fn transform_is_two_dimensional(value: &str) -> bool {
    parse_svg_transform_function_list(value).is_some_and(|functions| {
        functions.iter().all(|function| {
            matches!(
                function.name.to_ascii_lowercase().as_str(),
                "matrix"
                    | "translate"
                    | "translatex"
                    | "translatey"
                    | "scale"
                    | "scalex"
                    | "scaley"
                    | "rotate"
                    | "rotatez"
                    | "skew"
                    | "skewx"
                    | "skewy"
            )
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvgTransformOperation {
    Matrix([f32; 6]),
    Translate([f32; 2]),
    Scale([f32; 2]),
    Rotate { degrees: f32, origin: [f32; 2] },
    SkewX(f32),
    SkewY(f32),
}

pub fn transform_attribute_operations(source: &str) -> Option<Vec<SvgTransformOperation>> {
    if source.trim().is_empty() {
        return Some(Vec::new());
    }
    parse_svg_transform_function_list(source)?
        .into_iter()
        .map(|function| {
            let args = function
                .args
                .iter()
                .map(|arg| {
                    crate::values::parse_value::<style::values::specified::Number>(arg)
                        .map(|value| value.get())
                })
                .collect::<Option<Vec<_>>>()?;
            Some(
                match (function.name.to_ascii_lowercase().as_str(), args.as_slice()) {
                    ("matrix", [a, b, c, d, e, f]) => {
                        SvgTransformOperation::Matrix([*a, *b, *c, *d, *e, *f])
                    },
                    ("translate", [x]) => SvgTransformOperation::Translate([*x, 0.0]),
                    ("translate", [x, y]) => SvgTransformOperation::Translate([*x, *y]),
                    ("scale", [x]) => SvgTransformOperation::Scale([*x, *x]),
                    ("scale", [x, y]) => SvgTransformOperation::Scale([*x, *y]),
                    ("rotate", [degrees]) => SvgTransformOperation::Rotate {
                        degrees: *degrees,
                        origin: [0.0; 2],
                    },
                    ("rotate", [degrees, x, y]) => SvgTransformOperation::Rotate {
                        degrees: *degrees,
                        origin: [*x, *y],
                    },
                    ("skewx", [degrees]) => SvgTransformOperation::SkewX(*degrees),
                    ("skewy", [degrees]) => SvgTransformOperation::SkewY(*degrees),
                    _ => return None,
                },
            )
        })
        .collect()
}
