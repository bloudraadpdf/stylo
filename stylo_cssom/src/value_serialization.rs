#[derive(Clone, Debug, Eq, PartialEq)]
/// A resolved observable value cannot become property-value parser input.
///
/// ```compile_fail
/// fn parse(_: &str) {}
/// fn reject(output: stylo_cssom::value_serialization::ResolvedValueSerialization) {
///     parse(output);
/// }
/// ```
pub struct ResolvedValueSerialization(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedOmDeclaredValue {
    Serialized(ResolvedValueSerialization),
    PendingSubstitution,
}

impl ResolvedValueSerialization {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_css_text(self) -> String {
        self.0
    }
}
