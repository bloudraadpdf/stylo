use std::borrow::Cow;

use cssparser::{Delimiter, Parser, ParserInput, parse_important};

use crate::css_scan::{hex_encode, rewrite_style_rules, split_top_level};

pub const INTERNAL_NAME_PROPERTY: &str = "--moegoe-view-transition-name";
pub const INTERNAL_SCOPE_PROPERTY: &str = "--moegoe-view-transition-name-scope";

const AUTHORED_PROPERTY: &str = "view-transition-name";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewTransitionNameTreeScope(String);

impl ViewTransitionNameTreeScope {
    pub fn document() -> Self {
        Self("document".to_owned())
    }
    pub fn detached() -> Self {
        Self("detached".to_owned())
    }
    pub fn shadow(identity: &str) -> Self {
        Self(format!("shadow-{}", hex_encode(identity)))
    }

    #[must_use]
    pub fn parse_computed(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
        .then(|| Self(value.to_owned()))
    }

    fn as_css(&self) -> &str {
        &self.0
    }
}

fn declaration_is_important(value: &str) -> bool {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    if parser
        .parse_until_before(
            Delimiter::Bang,
            |input| -> Result<(), cssparser::ParseError<'_, ()>> {
                while input.next_including_whitespace_and_comments().is_ok() {}
                Ok(())
            },
        )
        .is_err()
        || parser.is_exhausted()
    {
        return false;
    }
    parse_important(&mut parser).is_ok() && parser.is_exhausted()
}

fn mirror_name_declarations(body: &str, scope: &ViewTransitionNameTreeScope) -> String {
    let mut output = String::with_capacity(body.len());
    for range in split_top_level(body.as_bytes(), b';') {
        let declaration = &body[range];
        output.push_str(declaration);
        output.push(';');
        let Some(colon) = declaration.find(':') else {
            continue;
        };
        if !declaration[..colon]
            .trim()
            .eq_ignore_ascii_case(AUTHORED_PROPERTY)
        {
            continue;
        }
        let value = &declaration[colon + 1..];
        output.push_str(INTERNAL_NAME_PROPERTY);
        output.push(':');
        output.push_str(value);
        output.push(';');
        output.push_str(INTERNAL_SCOPE_PROPERTY);
        output.push(':');
        output.push_str(scope.as_css());
        if declaration_is_important(value) {
            output.push_str(" !important");
        }
        output.push(';');
    }
    output
}

pub fn rewrite_view_transition_names<'a>(
    css: &'a str,
    scope: &ViewTransitionNameTreeScope,
) -> Cow<'a, str> {
    if !css
        .as_bytes()
        .windows(AUTHORED_PROPERTY.len())
        .any(|window| window.eq_ignore_ascii_case(AUTHORED_PROPERTY.as_bytes()))
    {
        return Cow::Borrowed(css);
    }
    let rewritten = rewrite_style_rules(css, &|prelude, body| {
        format!("{prelude}{{{}}}", mirror_name_declarations(body, scope))
    });
    Cow::Owned(rewritten)
}
