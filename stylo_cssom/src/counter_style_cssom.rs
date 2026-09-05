use cssparser::{Parser, ParserInput};
use style::{
    counter_style::{
        AdditiveSymbols, CounterRanges, Fallback, Negative, Pad, Symbol, Symbols, System,
        parse_counter_style_name_definition,
    },
    parser::Parse,
    stylesheets::CssRuleType,
};
use style_traits::ToCss;

fn parse_descriptor<T: Parse>(css: &str) -> Option<T> {
    crate::style_fragment_parser::parse_fragment_with_rule_type(
        css,
        CssRuleType::CounterStyle,
        T::parse,
    )
}

fn parse_name(css: &str) -> Option<style::values::CustomIdent> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_entirely(parse_counter_style_name_definition)
        .ok()
}

fn resolved_system(css: &str) -> Option<System> {
    if css.is_empty() {
        Some(System::Symbolic)
    } else {
        parse_descriptor(css)
    }
}

/// Apply one CSSOM `CSSCounterStyleRule` setter through Stylo's typed parser
/// and mutation checks. Invalid syntax and disallowed system changes are
/// represented by `None`, so the caller can leave the live rule unchanged.
pub fn mutate_counter_style_rule(
    node: &stylo_cssom_model::RuleNode,
    property: &str,
    value: &str,
) -> Option<stylo_cssom_model::RuleNode> {
    use stylo_cssom_model::RuleCssomData;

    let RuleCssomData::CounterStyle {
        mut name,
        mut system,
        mut negative,
        mut prefix,
        mut suffix,
        mut range,
        mut pad,
        mut fallback,
        mut symbols,
        additive_symbols: mut additive,
    } = node.cssom_data()?.clone()
    else {
        return None;
    };
    let canonical = |value: String| std::sync::Arc::<str>::from(value);
    match property {
        "name" => name = canonical(parse_name(value)?.to_css_string()),
        "system" => {
            let candidate = parse_descriptor::<System>(value)?.to_css_string();
            if system.split_ascii_whitespace().next() != candidate.split_ascii_whitespace().next() {
                return None;
            }
            system = canonical(candidate);
        },
        "negative" => negative = canonical(parse_descriptor::<Negative>(value)?.to_css_string()),
        "prefix" => prefix = canonical(parse_descriptor::<Symbol>(value)?.to_css_string()),
        "suffix" => suffix = canonical(parse_descriptor::<Symbol>(value)?.to_css_string()),
        "range" => range = canonical(parse_descriptor::<CounterRanges>(value)?.to_css_string()),
        "pad" => pad = canonical(parse_descriptor::<Pad>(value)?.to_css_string()),
        "fallback" => fallback = canonical(parse_descriptor::<Fallback>(value)?.to_css_string()),
        "symbols" => {
            let candidate = parse_descriptor::<Symbols>(value)?;
            match resolved_system(&system)? {
                System::Numeric | System::Alphabetic if candidate.0.len() < 2 => return None,
                System::Extends(_) => return None,
                System::Cyclic
                | System::Numeric
                | System::Alphabetic
                | System::Symbolic
                | System::Additive
                | System::Fixed { .. } => {},
            }
            symbols = canonical(candidate.to_css_string());
        },
        "additive-symbols" => {
            let candidate = parse_descriptor::<AdditiveSymbols>(value)?;
            if matches!(resolved_system(&system)?, System::Extends(_)) {
                return None;
            }
            additive = canonical(candidate.to_css_string());
        },
        _ => return None,
    }
    node.clone()
        .with_counter_style_data(RuleCssomData::CounterStyle {
            name,
            system,
            negative,
            prefix,
            suffix,
            range,
            pad,
            fallback,
            symbols,
            additive_symbols: additive,
        })
}

/// Return the CSSOM serialization of a `CSSCounterStyleRule` attribute.
pub fn counter_style_rule_value(
    node: &stylo_cssom_model::RuleNode,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    let stylo_cssom_model::RuleCssomData::CounterStyle {
        name,
        system,
        negative,
        prefix,
        suffix,
        range,
        pad,
        fallback,
        symbols,
        additive_symbols,
    } = node.cssom_data()?
    else {
        return None;
    };
    Some(crate::value_serialization::ResolvedValueSerialization::new(
        match property {
            "name" => name.to_string(),
            "system" => system.to_string(),
            "negative" => negative.to_string(),
            "prefix" => prefix.to_string(),
            "suffix" => suffix.to_string(),
            "range" => range.to_string(),
            "pad" => pad.to_string(),
            "fallback" => fallback.to_string(),
            "symbols" => symbols.to_string(),
            "additive-symbols" => additive_symbols.to_string(),
            _ => return None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{counter_style_rule_value, mutate_counter_style_rule};

    const FIXED: &str = "@counter-style foo { system: fixed; symbols: A B C; }";

    fn fixed_rule() -> stylo_cssom_model::RuleNode {
        crate::ValidatedCssRule::parse(crate::RuleInput::new(FIXED))
            .expect("the fixed counter style must parse")
            .into_rule_node()
    }

    #[test]
    fn typed_cssom_mutation_accepts_parameters_but_not_system_changes() {
        let fixed_zero = mutate_counter_style_rule(&fixed_rule(), "system", "fixed 0")
            .expect("valid fixed parameter");
        assert_eq!(
            counter_style_rule_value(&fixed_zero, "system").map(|value| value.into_css_text()),
            Some("fixed 0".to_owned())
        );
        assert!(mutate_counter_style_rule(&fixed_rule(), "system", "numeric").is_none());
        assert!(mutate_counter_style_rule(&fixed_rule(), "system", "123").is_none());
    }

    #[test]
    fn typed_cssom_mutation_preserves_a_rule_after_invalid_descriptor_syntax() {
        assert!(mutate_counter_style_rule(&fixed_rule(), "pad", "3 \"0\"").is_some());
        assert!(mutate_counter_style_rule(&fixed_rule(), "pad", "-1 \"0\"").is_none());
        assert!(mutate_counter_style_rule(&fixed_rule(), "name", "decimal").is_none());
    }

    #[test]
    fn typed_cssom_mutation_applies_system_dependent_symbol_checks() {
        let alphabetic = crate::ValidatedCssRule::parse(crate::RuleInput::new(
            "@counter-style foo { system: alphabetic; symbols: A B C; }",
        ))
        .expect("the alphabetic counter style must parse")
        .into_rule_node();

        assert!(mutate_counter_style_rule(&alphabetic, "symbols", "A").is_none());
        assert!(mutate_counter_style_rule(&alphabetic, "symbols", "A B").is_some());
    }
}
