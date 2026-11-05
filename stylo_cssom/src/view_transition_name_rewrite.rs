use std::borrow::Cow;

use cssparser::{Delimiter, Parser, ParserInput, parse_important};

use crate::css_scan::{hex_encode, rewrite_style_rules, split_top_level};

pub const INTERNAL_NAME_PROPERTY: &str = "--moegoe-view-transition-name";
pub const INTERNAL_SCOPE_PROPERTY: &str = "--moegoe-view-transition-name-scope";
pub const INTERNAL_CLASS_SCOPE_PROPERTY: &str = "--moegoe-view-transition-class-scope";

const AUTHORED_PROPERTY: &str = "view-transition-name";
const AUTHORED_CLASS_PROPERTY: &str = "view-transition-class";
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
        let property = declaration[..colon].trim();
        let value = &declaration[colon + 1..];
        let scope_property = if property.eq_ignore_ascii_case(AUTHORED_PROPERTY) {
            output.push_str(INTERNAL_NAME_PROPERTY);
            output.push(':');
            output.push_str(value);
            output.push(';');
            INTERNAL_SCOPE_PROPERTY
        } else if property.eq_ignore_ascii_case(AUTHORED_CLASS_PROPERTY) {
            INTERNAL_CLASS_SCOPE_PROPERTY
        } else {
            continue;
        };
        output.push_str(scope_property);
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
        && !css
            .as_bytes()
            .windows(AUTHORED_CLASS_PROPERTY.len())
            .any(|window| window.eq_ignore_ascii_case(AUTHORED_CLASS_PROPERTY.as_bytes()))
    {
        return Cow::Borrowed(css);
    }
    let rewritten = rewrite_style_rules(css, &|prelude, body| {
        format!("{prelude}{{{}}}", mirror_name_declarations(body, scope))
    });
    Cow::Owned(rewritten)
}

/// The declarations of a `style` attribute with each `view-transition-name` mirrored, as
/// `rewrite_view_transition_names` mirrors the declarations of a style sheet.
pub fn mirror_inline_view_transition_names(
    declarations: Vec<stylo_cssom_model::RuleDeclaration>,
    scope: &ViewTransitionNameTreeScope,
) -> Vec<stylo_cssom_model::RuleDeclaration> {
    declarations
        .into_iter()
        .flat_map(|declaration| {
            let important = declaration.important();
            let scope_declaration = |property| {
                stylo_cssom_model::RuleDeclaration::new(property, scope.as_css())
                    .with_importance(important)
            };
            let mirrored = if declaration.matches_name(AUTHORED_PROPERTY) {
                vec![
                    stylo_cssom_model::RuleDeclaration::new(
                        INTERNAL_NAME_PROPERTY,
                        declaration.value(),
                    )
                    .with_importance(important),
                    scope_declaration(INTERNAL_SCOPE_PROPERTY),
                ]
            } else if declaration.matches_name(AUTHORED_CLASS_PROPERTY) {
                vec![scope_declaration(INTERNAL_CLASS_SCOPE_PROPERTY)]
            } else {
                Vec::new()
            };
            std::iter::once(declaration).chain(mirrored)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_names_mirror_into_the_internal_properties() {
        let declarations = vec![
            stylo_cssom_model::RuleDeclaration::new("color", "red"),
            stylo_cssom_model::RuleDeclaration::new(AUTHORED_PROPERTY, "card")
                .with_importance(true),
        ];
        let mirrored = mirror_inline_view_transition_names(
            declarations,
            &ViewTransitionNameTreeScope::document(),
        )
        .iter()
        .map(|declaration| {
            format!(
                "{}:{}:{}",
                declaration.name(),
                declaration.value(),
                declaration.important()
            )
        })
        .collect::<Vec<_>>();
        assert_eq!(
            mirrored,
            [
                "color:red:false",
                "view-transition-name:card:true",
                "--moegoe-view-transition-name:card:true",
                "--moegoe-view-transition-name-scope:document:true",
            ]
        );
    }

    #[test]
    fn classes_record_their_declaration_tree_scope() {
        let shadow = ViewTransitionNameTreeScope::shadow("host");
        let mirrored = mirror_inline_view_transition_names(
            vec![stylo_cssom_model::RuleDeclaration::new(
                AUTHORED_CLASS_PROPERTY,
                "red",
            )],
            &shadow,
        )
        .iter()
        .map(|declaration| format!("{}:{}", declaration.name(), declaration.value()))
        .collect::<Vec<_>>();
        assert_eq!(
            mirrored,
            [
                "view-transition-class:red".to_owned(),
                format!("--moegoe-view-transition-class-scope:{}", shadow.as_css()),
            ]
        );
        let rewritten = rewrite_view_transition_names("div{view-transition-class:red}", &shadow);
        assert!(rewritten.contains(&format!(
            "--moegoe-view-transition-class-scope:{}",
            shadow.as_css()
        )));
    }
}
