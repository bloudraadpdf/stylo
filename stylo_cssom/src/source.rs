use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};
use stylo_cssom_model::Importance;

#[derive(Debug)]
pub struct SourceDeclaration<'a> {
    pub property: String,
    pub value: &'a str,
    pub important: bool,
    pub source_value: &'a str,
    pub line: u32,
    pub column: u32,
    pub at_rules: Vec<String>,
}

pub fn declarations(source: &str) -> Vec<SourceDeclaration<'_>> {
    let mut input = ParserInput::new(source);
    let mut scanner = DeclarationScanner {
        declarations: Vec::new(),
        at_rules: Vec::new(),
    };
    for _ in RuleBodyParser::new(&mut Parser::new(&mut input), &mut scanner) {}
    scanner.declarations
}

pub fn stylesheet_declarations(source: &str) -> Vec<SourceDeclaration<'_>> {
    let mut input = ParserInput::new(source);
    let mut scanner = DeclarationScanner {
        declarations: Vec::new(),
        at_rules: Vec::new(),
    };
    for _ in StyleSheetParser::new(&mut Parser::new(&mut input), &mut scanner) {}
    scanner.declarations
}

pub fn declaration_value<'a>(source: &'a str, property: &str) -> Option<&'a str> {
    declarations(source)
        .into_iter()
        .find(|declaration| declaration.property.eq_ignore_ascii_case(property))
        .map(|declaration| declaration.value)
}

pub fn strip_declarations(source: &str, properties: &[&str]) -> String {
    declarations(source)
        .into_iter()
        .filter(|declaration| {
            !properties
                .iter()
                .any(|property| declaration.property.eq_ignore_ascii_case(property))
        })
        .map(|declaration| serialize_declaration(&declaration))
        .collect::<Vec<_>>()
        .join("; ")
}

fn serialize_declaration(declaration: &SourceDeclaration<'_>) -> String {
    let important = if declaration.important {
        " !important"
    } else {
        ""
    };
    format!("{}: {}{important}", declaration.property, declaration.value)
}

pub fn replace_declaration(source: &str, property: &str, value: &str) -> String {
    let mut declarations = declarations(source)
        .into_iter()
        .filter(|declaration| !declaration.property.eq_ignore_ascii_case(property))
        .map(|declaration| serialize_declaration(&declaration))
        .collect::<Vec<_>>();
    declarations.push(format!("{property}: {value}"));
    declarations.join("; ")
}

pub fn substitute_variables(
    source: &str,
    resolve: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    use crate::typed_om::{
        TypedOmUnparsedInput, TypedOmUnparsedSegment, TypedOmUnparsedValue,
        parse_typed_om_unparsed_value,
    };
    fn substitute(
        value: &TypedOmUnparsedValue,
        resolve: &impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        let mut output = String::new();
        for segment in value.segments() {
            match segment {
                TypedOmUnparsedSegment::String(text) => output.push_str(text),
                TypedOmUnparsedSegment::VariableReference(reference) => {
                    let value = resolve(reference.name())
                        .or_else(|| substitute(reference.fallback()?, resolve))?;
                    output.push_str(&value);
                },
            }
        }
        Some(output)
    }
    let value = parse_typed_om_unparsed_value(TypedOmUnparsedInput::new(source))?;
    value
        .contains_variable_reference()
        .then(|| substitute(&value, resolve))
        .flatten()
}

pub enum InlineClipPathReference {
    Absent,
    Local(String),
    Other,
}

pub fn local_url_fragment(source: &str) -> Option<String> {
    let url = crate::values::parse_value::<style::values::specified::url::SpecifiedUrl>(source)?;
    url.original()?
        .strip_prefix('#')
        .filter(|fragment| !fragment.is_empty())
        .map(str::to_owned)
}

pub fn inline_clip_path_reference(source: &str) -> InlineClipPathReference {
    let block = crate::declaration_parser::parse_inline_style_block(source);
    let Some(value) =
        crate::declaration_parser::inline_style_get_property_value(&block, "clip-path")
    else {
        return InlineClipPathReference::Absent;
    };
    local_url_fragment(&value).map_or(
        InlineClipPathReference::Other,
        InlineClipPathReference::Local,
    )
}

struct DeclarationScanner<'i> {
    declarations: Vec<SourceDeclaration<'i>>,
    at_rules: Vec<String>,
}

impl<'i> DeclarationScanner<'i> {
    fn scan_rule_body(&mut self, input: &mut Parser<'i, '_>) {
        for _ in RuleBodyParser::new(input, self) {}
    }

    fn consume_component_values(input: &mut Parser<'_, '_>) {
        while input.next_including_whitespace_and_comments().is_ok() {}
    }
}

impl<'i> DeclarationParser<'i> for DeclarationScanner<'i> {
    type Declaration = ();
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let value_start = input.position();
        Self::consume_component_values(input);
        let source_value = input.slice_from(value_start);
        let (value, importance) =
            crate::declaration_parser::split_inline_declaration_importance(source_value);
        let location = declaration_start.source_location();
        self.declarations.push(SourceDeclaration {
            property: if name.starts_with("--") {
                name.to_string()
            } else {
                name.to_ascii_lowercase()
            },
            value: value.trim(),
            source_value,
            at_rules: self.at_rules.clone(),
            important: importance == Importance::Important,
            line: location.line,
            column: location.column,
        });
        Ok(())
    }
}

impl<'i> QualifiedRuleParser<'i> for DeclarationScanner<'i> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        Self::consume_component_values(input);
        Ok(())
    }

    fn parse_block<'t>(
        &mut self,
        (): Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        self.scan_rule_body(input);
        Ok(())
    }
}

impl<'i> AtRuleParser<'i> for DeclarationScanner<'i> {
    type Prelude = String;
    type AtRule = ();
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        Self::consume_component_values(input);
        Ok(name.to_ascii_lowercase())
    }

    fn rule_without_block(
        &mut self,
        _: Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        Ok(())
    }

    fn parse_block<'t>(
        &mut self,
        name: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        self.at_rules.push(name);
        self.scan_rule_body(input);
        self.at_rules.pop();
        Ok(())
    }
}

impl<'i> RuleBodyItemParser<'i, (), ()> for DeclarationScanner<'i> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_declarations_preserve_nested_values_and_custom_property_case() {
        let source = r#"--Case: "a;b"; width: calc(1px + var(--Case, 2px)) !important; color: red"#;
        let values = declarations(source);
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].property, "--Case");
        assert_eq!(values[0].value, r#""a;b""#);
        assert_eq!(values[1].value, "calc(1px + var(--Case, 2px))");
        assert!(values[1].important);
    }

    #[test]
    fn declaration_analysis_keeps_page_rule_ancestry() {
        let values = stylesheet_declarations(
            "@page { margin: 0; @top-left { content: 'x'; } } @media print { p { color: red } }",
        );
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].at_rules, ["page"]);
        assert_eq!(values[1].at_rules, ["page", "top-left"]);
        assert_eq!(values[2].at_rules, ["media"]);
    }

    #[test]
    fn inline_clip_path_reference_uses_the_winning_valid_declaration() {
        assert!(
            matches!(inline_clip_path_reference("clip-path: url('#local')"), InlineClipPathReference::Local(value) if value == "local")
        );
        assert!(
            matches!(inline_clip_path_reference("clip-path: url(#local) !important; clip-path: none"), InlineClipPathReference::Local(value) if value == "local")
        );
        assert!(matches!(
            inline_clip_path_reference("clip-path: url(#local); clip-path: none"),
            InlineClipPathReference::Other
        ));
        assert!(matches!(
            inline_clip_path_reference("clip-path: invalid"),
            InlineClipPathReference::Absent
        ));
        assert!(matches!(
            inline_clip_path_reference("clip-path: url(other.svg#local)"),
            InlineClipPathReference::Other
        ));
    }
}
