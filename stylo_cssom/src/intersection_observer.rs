use crate::numeric::{CanonicalUnitDimension, CssUnitKind, TypedOmUnit};
use cssparser::{Parser, ParserInput, Token};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IntersectionObserverMarginValue {
    Pixels(f64),
    Percentage(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntersectionObserverMargin([IntersectionObserverMarginValue; 4]);

impl IntersectionObserverMargin {
    pub fn sides(&self) -> [IntersectionObserverMarginValue; 4] {
        self.0
    }

    pub fn serialize(&self) -> String {
        self.0
            .map(|side| match side {
                IntersectionObserverMarginValue::Pixels(value) => format!("{}px", value.trunc()),
                IntersectionObserverMarginValue::Percentage(value) => {
                    format!("{}%", (value * 100.0) as f32)
                },
            })
            .join(" ")
    }
}

pub fn parse_intersection_observer_margin(input: &str) -> Option<IntersectionObserverMargin> {
    let mut input = ParserInput::new(input);
    let mut parser = Parser::new(&mut input);
    let mut values = Vec::with_capacity(4);
    while !parser.is_exhausted() {
        if values.len() == 4 {
            return None;
        }
        let value = match parser.next().ok()? {
            Token::Percentage { unit_value, .. } if unit_value.is_finite() => {
                IntersectionObserverMarginValue::Percentage(unit_value.to_string().parse().ok()?)
            },
            Token::Dimension { value, unit, .. } if value.is_finite() => {
                let CssUnitKind::Canonical(conversion) = TypedOmUnit::parse(unit)?.kind() else {
                    return None;
                };
                if conversion.dimension != CanonicalUnitDimension::AbsoluteLength {
                    return None;
                }
                IntersectionObserverMarginValue::Pixels(f64::from(*value) * conversion.scale)
            },
            _ => return None,
        };
        values.push(value);
    }
    use IntersectionObserverMarginValue::Pixels;
    let sides = match values.as_slice() {
        [] => [Pixels(0.0); 4],
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    };
    Some(IntersectionObserverMargin(sides))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_units_match_web_platform_margin_serialization() {
        for (unit, expected) in [
            ("px", "10px"),
            ("cm", "377px"),
            ("mm", "37px"),
            ("q", "9px"),
            ("in", "960px"),
            ("pt", "13px"),
            ("pc", "160px"),
        ] {
            let margin = parse_intersection_observer_margin(&format!("10{unit}")).unwrap();
            assert_eq!(margin.serialize(), [expected; 4].join(" "));
        }
    }

    #[test]
    fn margin_shorthand_retains_typed_percentages_and_lengths() {
        let margin = parse_intersection_observer_margin("1px 25% -3px").unwrap();
        assert_eq!(
            margin.sides(),
            [
                IntersectionObserverMarginValue::Pixels(1.0),
                IntersectionObserverMarginValue::Percentage(0.25),
                IntersectionObserverMarginValue::Pixels(-3.0),
                IntersectionObserverMarginValue::Percentage(0.25)
            ]
        );
        assert_eq!(margin.serialize(), "1px 25% -3px 25%");
        assert_eq!(
            parse_intersection_observer_margin("20%").unwrap().sides()[0],
            IntersectionObserverMarginValue::Percentage(0.2)
        );
        assert_eq!(
            parse_intersection_observer_margin("").unwrap().serialize(),
            "0px 0px 0px 0px"
        );
        assert!(parse_intersection_observer_margin("1PX /* margin */ 2%").is_some());
    }

    #[test]
    fn margin_rejects_non_absolute_lengths_and_invalid_lists() {
        for input in [
            "0",
            "1em",
            "1vw",
            "1deg",
            "auto",
            "calc(1px)",
            "1px, 2px",
            "1px 2px 3px 4px 5px",
            "1px garbage",
        ] {
            assert!(
                parse_intersection_observer_margin(input).is_none(),
                "{input}"
            );
        }
    }
}
