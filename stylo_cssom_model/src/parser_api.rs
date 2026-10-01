use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxBlockKind {
    Parentheses,
    Brackets,
    Braces,
}

impl SyntaxBlockKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Parentheses => "()",
            Self::Brackets => "[]",
            Self::Braces => "{}",
        }
    }

    pub fn from_name(name: &str) -> Result<Self, SyntaxParseError> {
        match name {
            "()" => Ok(Self::Parentheses),
            "[]" => Ok(Self::Brackets),
            "{}" => Ok(Self::Braces),
            _ => Err(SyntaxParseError::InvalidBlockName),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyntaxValue {
    Token(Arc<str>),
    Block {
        kind: SyntaxBlockKind,
        body: Vec<Self>,
    },
    Function {
        name: Arc<str>,
        args: Vec<Vec<Self>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyntaxRule {
    AtRule {
        name: Arc<str>,
        prelude: Vec<SyntaxValue>,
        body: Option<Vec<Self>>,
    },
    QualifiedRule {
        prelude: Vec<SyntaxValue>,
        body: Vec<Self>,
    },
    Declaration {
        name: Arc<str>,
        body: Vec<SyntaxValue>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SyntaxBodyKind {
    Rules,
    #[default]
    Declarations,
}

#[derive(Clone, Debug, Default)]
pub struct SyntaxOptions {
    at_rules: BTreeMap<String, SyntaxBodyKind>,
}

impl SyntaxOptions {
    pub fn set_at_rule(&mut self, name: &str, kind: SyntaxBodyKind) {
        self.at_rules.insert(name.to_ascii_lowercase(), kind);
    }

    pub fn at_rule_body(&self, name: &str) -> SyntaxBodyKind {
        self.at_rules
            .get(&name.to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyntaxParseError {
    #[error("expected one CSS rule")]
    InvalidRule,
    #[error("expected a CSS declaration")]
    InvalidDeclaration,
    #[error("expected one CSS component value")]
    InvalidValue,
    #[error("block name must be (), [] or {{}}")]
    InvalidBlockName,
    #[error("CSS syntax nesting limit exceeded")]
    NestingLimit,
}
