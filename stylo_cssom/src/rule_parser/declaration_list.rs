use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Parser, ParserState, QualifiedRuleParser,
    RuleBodyItemParser, RuleBodyParser,
};

pub fn parse<'i, T, E: 'i>(
    input: &mut Parser<'i, '_>,
    mut parse_value: impl for<'t> FnMut(
        CowRcStr<'i>,
        &mut Parser<'i, 't>,
        &ParserState,
    ) -> Result<T, cssparser::ParseError<'i, E>>,
) -> Vec<T> {
    RuleBodyParser::new(
        input,
        &mut DeclarationList {
            parse_value: &mut parse_value,
        },
    )
    .filter_map(Result::ok)
    .collect()
}

type ValueParser<'a, 'i, T, E> = dyn for<'t> FnMut(
        CowRcStr<'i>,
        &mut Parser<'i, 't>,
        &ParserState,
    ) -> Result<T, cssparser::ParseError<'i, E>>
    + 'a;

struct DeclarationList<'a, 'i, T, E> {
    parse_value: &'a mut ValueParser<'a, 'i, T, E>,
}

impl<'i, T, E> DeclarationParser<'i> for DeclarationList<'_, 'i, T, E> {
    type Declaration = T;
    type Error = E;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        start: &ParserState,
    ) -> Result<T, cssparser::ParseError<'i, E>> {
        (self.parse_value)(name, input, start)
    }
}

impl<'i, T, E> AtRuleParser<'i> for DeclarationList<'_, 'i, T, E> {
    type Prelude = ();
    type AtRule = T;
    type Error = E;
}

impl<'i, T, E> QualifiedRuleParser<'i> for DeclarationList<'_, 'i, T, E> {
    type Prelude = ();
    type QualifiedRule = T;
    type Error = E;
}

impl<'i, T, E> RuleBodyItemParser<'i, T, E> for DeclarationList<'_, 'i, T, E> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}
