use cssparser::{Parser, ParserInput, Token, serialize_identifier};
use style::values::CustomIdent;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewTransitionNameIdentity(String);

impl ViewTransitionNameIdentity {
    pub fn parse_computed(css: &str) -> Option<Self> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| {
                CustomIdent::parse(input, &["none", "auto", "match-element"])
                    .map(|name| Self(name.0.as_ref().to_owned()))
            })
            .ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn to_css_identifier(&self) -> String {
        let mut css = String::new();
        serialize_identifier(&self.0, &mut css).expect("serializing into a String cannot fail");
        css
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedViewTransitionGroup {
    Normal,
    Contain,
    Nearest,
    Named(ViewTransitionNameIdentity),
}

impl ComputedViewTransitionGroup {
    pub fn parse(css: &str) -> Option<Self> {
        match css.trim() {
            "normal" => Some(Self::Normal),
            "contain" => Some(Self::Contain),
            "nearest" => Some(Self::Nearest),
            name => {
                let mut input = ParserInput::new(name);
                let mut parser = Parser::new(&mut input);
                parser
                    .parse_entirely(|input| {
                        CustomIdent::parse(input, &["normal", "contain", "nearest", "none"]).map(
                            |name| {
                                Self::Named(ViewTransitionNameIdentity(name.0.as_ref().to_owned()))
                            },
                        )
                    })
                    .ok()
            },
        }
    }

    pub const fn establishes_containing_group(&self) -> bool {
        !matches!(self, Self::Normal)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedViewTransitionGroupParent {
    TransitionRoot,
    Named(ViewTransitionNameIdentity),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewTransitionClassList(Vec<String>);

impl ViewTransitionClassList {
    pub fn parse_computed(css: &str) -> Option<Self> {
        if css.eq_ignore_ascii_case("none") {
            return Some(Self::default());
        }
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| {
                let mut classes = Vec::new();
                while !input.is_exhausted() {
                    classes.push(CustomIdent::parse(input, &["none"])?.0.as_ref().to_owned());
                }
                Ok(Self(classes))
            })
            .ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewTransitionUaAnimation {
    FadeOut,
    FadeIn,
    PlusLighter,
}

impl ViewTransitionUaAnimation {
    pub const ALL: [Self; 3] = [Self::FadeOut, Self::FadeIn, Self::PlusLighter];

    const fn identifier_and_frames(
        self,
    ) -> (
        &'static str,
        &'static [(&'static str, &'static str, &'static str)],
    ) {
        match self {
            Self::FadeOut => ("-ua-view-transition-fade-out", &[("to", "opacity", "0")]),
            Self::FadeIn => ("-ua-view-transition-fade-in", &[("from", "opacity", "0")]),
            Self::PlusLighter => (
                "-ua-mix-blend-mode-plus-lighter",
                &[
                    ("from", "mix-blend-mode", "plus-lighter"),
                    ("to", "mix-blend-mode", "plus-lighter"),
                ],
            ),
        }
    }

    pub const fn identifier(self) -> &'static str {
        self.identifier_and_frames().0
    }

    fn rule(self) -> stylo_cssom_model::RuleNode {
        let (identifier, frames) = self.identifier_and_frames();
        let frames = frames
            .iter()
            .map(|(selector, property, value)| {
                stylo_cssom_model::RuleNode::keyframe(
                    *selector,
                    [stylo_cssom_model::RuleDeclaration::new(*property, *value)],
                )
            })
            .collect::<Vec<_>>();
        stylo_cssom_model::RuleNode::keyframes(identifier, frames)
            .expect("the view-transition UA animation contains only keyframe entries")
    }

    pub fn stylesheet_root() -> stylo_cssom_model::InternalStylesheetRoot {
        stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::UserAgent,
            Self::ALL.into_iter().map(Self::rule).collect::<Vec<_>>(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationViewTransition {
    #[default]
    None,
    Auto,
}

pub fn parse_navigation_view_transition(css: &str) -> NavigationViewTransition {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        let is_view_transition = matches!(
            token.clone(),
            Token::AtKeyword(name) if name.eq_ignore_ascii_case("view-transition")
        );
        if is_view_transition && consume_at_rule(&mut parser) {
            return NavigationViewTransition::Auto;
        }
    }
    NavigationViewTransition::None
}

fn consume_at_rule<'i>(parser: &mut Parser<'i, '_>) -> bool {
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token.clone() {
            Token::Semicolon => return false,
            Token::CurlyBracketBlock => {
                let parsed = parser.parse_nested_block(
                    |inner| -> Result<bool, cssparser::ParseError<'i, ()>> {
                        Ok(contains_auto_navigation(inner))
                    },
                );
                return parsed.unwrap_or(false);
            },
            _ => {},
        }
    }
    false
}

fn contains_auto_navigation(parser: &mut Parser<'_, '_>) -> bool {
    let mut found = false;
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        let Token::Ident(name) = token.clone() else {
            continue;
        };
        if !name.eq_ignore_ascii_case("navigation") {
            continue;
        }
        parser.skip_whitespace();
        if parser.expect_colon().is_err() {
            continue;
        }
        parser.skip_whitespace();
        if matches!(parser.next(), Ok(Token::Ident(value)) if value.eq_ignore_ascii_case("auto")) {
            found = true;
        }
    }
    found
}
