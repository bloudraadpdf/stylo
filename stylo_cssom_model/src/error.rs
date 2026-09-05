use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum CssomStylesheetError {
    #[error("CSS rule list contains an unterminated string, comment, or block")]
    Unterminated,
    #[error("CSS rule insertion index {index} exceeds rule count {len}")]
    InvalidInsertionIndex { index: usize, len: usize },
    #[error("CSS rule deletion index {index} does not name one of {len} rules")]
    InvalidDeletionIndex { index: usize, len: usize },
    #[error("insertRule requires exactly one complete CSS rule")]
    ExpectedSingleRule,
}
