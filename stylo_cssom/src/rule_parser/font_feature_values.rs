use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, Token,
};
use style::stylesheets::{
    CssRuleType, FontFeatureValuesRule, font_feature_values_rule::parse_family_name_list,
};
use style_traits::ToCss;
use stylo_cssom_model::{
    FontFeatureKind, FontFeatureMap, RuleCssomData, RuleFontFeatureValues, RuleGrammar, RuleNode,
};

pub(super) fn canonical_rule(
    rule: &FontFeatureValuesRule,
    authored: &super::source::AuthoredSource<'_>,
) -> RuleFontFeatureValues {
    let family = rule
        .family_names
        .iter()
        .map(ToCss::to_css_string)
        .collect::<Vec<_>>()
        .join(", ");
    let mut values = RuleFontFeatureValues::new(family);
    for (kind, entries) in [
        (FontFeatureKind::Annotation, &rule.annotation),
        (FontFeatureKind::Ornaments, &rule.ornaments),
        (FontFeatureKind::Stylistic, &rule.stylistic),
        (FontFeatureKind::Swash, &rule.swash),
    ] {
        for entry in entries {
            values
                .map_mut(kind)
                .set(entry.name.as_ref(), vec![entry.value.0])
                .expect("parsed feature value has its required arity");
        }
    }
    for entry in &rule.character_variant {
        values
            .map_mut(FontFeatureKind::CharacterVariant)
            .set(
                entry.name.as_ref(),
                std::iter::once(entry.value.0)
                    .chain(entry.value.1)
                    .collect::<Vec<_>>(),
            )
            .expect("parsed character variant has one or two values");
    }
    for entry in &rule.styleset {
        values
            .map_mut(FontFeatureKind::Styleset)
            .set(entry.name.as_ref(), entry.value.0.clone())
            .expect("parsed styleset has at least one value");
    }
    let source = authored.span(rule.source_location);
    let source = source.as_ref().map(|span| span.text(authored.text()));
    if let Some(body) = source.and_then(super::outer_block_contents) {
        let mut input = ParserInput::new(&body);
        let mut input = Parser::new(&mut input);
        let mut parser = HistoricalBlocks(values.map_mut(FontFeatureKind::HistoricalForms));
        for _ in RuleBodyParser::new(&mut input, &mut parser) {}
    }
    values
}

struct HistoricalBlocks<'a>(&'a mut FontFeatureMap);
impl<'i> AtRuleParser<'i> for HistoricalBlocks<'_> {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        if !name.eq_ignore_ascii_case("historical-forms") {
            return Err(input.new_custom_error(()));
        }
        input.expect_exhausted()?;
        Ok(())
    }
    fn parse_block<'t>(
        &mut self,
        _: (),
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        let _ = super::declaration_list::parse(
            input,
            |name, input, _| -> Result<(), cssparser::ParseError<'_, ()>> {
                let value = match *input.next()? {
                    Token::Number {
                        int_value: Some(value),
                        ..
                    } if value >= 0 => value as u32,
                    _ => return Err(input.new_custom_error(())),
                };
                input.expect_exhausted()?;
                self.0
                    .set(name.as_ref(), vec![value])
                    .expect("historical forms accept one integer");
                Ok(())
            },
        );
        Ok(())
    }
}
impl<'i> DeclarationParser<'i> for HistoricalBlocks<'_> {
    type Declaration = ();
    type Error = ();
}
impl<'i> QualifiedRuleParser<'i> for HistoricalBlocks<'_> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}
impl<'i> RuleBodyItemParser<'i, (), ()> for HistoricalBlocks<'_> {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        false
    }
}

pub fn font_feature_values_node(values: RuleFontFeatureValues) -> RuleNode {
    let mut text = format!("@font-feature-values {} {{\n", values.font_family());
    for kind in FontFeatureKind::ALL {
        let map = values.map(kind);
        if map.is_empty() {
            continue;
        }
        text.push_str(&format!("  @{} {{\n", kind.at_keyword()));
        let mut cursor = 0;
        while let Some((next, name, indices)) = map.next_entry(cursor) {
            cursor = next;
            text.push_str("    ");
            cssparser::serialize_identifier(name, &mut text).expect("writing a string cannot fail");
            text.push_str(": ");
            text.push_str(
                &indices
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            text.push_str(";\n");
        }
        text.push_str("  }\n");
    }
    text.push('}');
    RuleNode::authored(RuleGrammar::FontFeatureValues, text, [])
        .with_cssom_data(RuleCssomData::FontFeatureValues { values })
        .expect("feature values match their rule grammar")
}

pub fn replace_font_feature_family(node: &RuleNode, family: &str) -> Option<RuleNode> {
    let RuleCssomData::FontFeatureValues { values } = node.cssom_data()? else {
        return None;
    };
    let family = crate::style_fragment_parser::parse_fragment_with_rule_type(
        family,
        CssRuleType::FontFeatureValues,
        parse_family_name_list,
    )?;
    let mut values = values.clone();
    values.set_font_family(
        family
            .iter()
            .map(ToCss::to_css_string)
            .collect::<Vec<_>>()
            .join(", "),
    );
    Some(font_feature_values_node(values))
}
