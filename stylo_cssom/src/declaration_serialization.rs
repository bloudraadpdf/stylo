use crate::declaration_parser::{CssomDeclarationBlock, InlineStyleBlock};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Observable declaration text cannot become declaration parser input.
///
/// ```compile_fail
/// fn parse(_: stylo_cssom::declaration_parser::DeclarationInput<'_>) {}
/// fn reject(output: stylo_cssom::declaration_serialization::DeclarationSerialization) {
///     parse(output);
/// }
/// ```
pub struct DeclarationSerialization(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationEntrySerialization {
    name: String,
    value: crate::value_serialization::ResolvedValueSerialization,
    important: bool,
}

impl DeclarationEntrySerialization {
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        String,
        crate::value_serialization::ResolvedValueSerialization,
        bool,
    ) {
        (self.name, self.value, self.important)
    }
}

impl DeclarationSerialization {
    #[must_use]
    pub fn into_css_text(self) -> String {
        self.0
    }
}

pub fn serialise_inline_style_block(block: &InlineStyleBlock) -> DeclarationSerialization {
    serialise_typed_declaration_block(block.as_typed())
}

#[must_use]
pub fn serialise_inline_style_projection(
    projection: &stylo_cssom_model::InlineStyleProjection,
) -> DeclarationSerialization {
    serialise_specified_declarations(projection.declarations())
}

pub fn serialise_specified_declarations(
    declarations: &[stylo_cssom_model::SpecifiedDeclaration],
) -> DeclarationSerialization {
    DeclarationSerialization(crate::specified::serialize_specified_declarations(
        declarations,
    ))
}

fn serialise_typed_declaration_block(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
) -> DeclarationSerialization {
    let mut out = String::new();
    let _ = block.to_css(&mut out);
    DeclarationSerialization(out)
}

pub fn serialise_cssom_declaration_block(
    block: &CssomDeclarationBlock,
) -> DeclarationSerialization {
    serialise_typed_declaration_block(block.as_typed())
}

#[must_use]
pub fn serialise_rule_declaration_block(
    block: &stylo_cssom_model::RuleBlock,
) -> DeclarationSerialization {
    DeclarationSerialization(block.serialization().to_string())
}

#[must_use]
pub fn serialise_inline_style_property_value(
    block: &InlineStyleBlock,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    crate::declaration_parser::inline_style_get_property_value(block, property)
        .map(crate::value_serialization::ResolvedValueSerialization::new)
}

#[must_use]
pub fn remove_and_serialise_inline_style_property(
    block: &mut InlineStyleBlock,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    crate::declaration_parser::inline_style_remove_property(block, property)
        .map(crate::value_serialization::ResolvedValueSerialization::new)
}

#[must_use]
pub fn serialise_inline_style_cssom_property_value(
    block: &InlineStyleBlock,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    serialise_authored_inline_style_value(
        property,
        serialise_inline_style_property_value(
            block,
            crate::declaration_parser::inline_style_cssom_backing_property(property),
        ),
    )
}

#[must_use]
pub fn remove_and_serialise_inline_style_cssom_property(
    block: &mut InlineStyleBlock,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    serialise_authored_inline_style_value(
        property,
        remove_and_serialise_inline_style_property(
            block,
            crate::declaration_parser::inline_style_cssom_backing_property(property),
        ),
    )
}

fn serialise_authored_inline_style_value(
    property: &str,
    value: Option<crate::value_serialization::ResolvedValueSerialization>,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    crate::declaration_parser::inline_style_cssom_authored_value(
        property,
        value.map(crate::value_serialization::ResolvedValueSerialization::into_css_text),
    )
    .map(crate::value_serialization::ResolvedValueSerialization::new)
}

#[must_use]
pub fn serialise_inline_style_declarations(
    block: &InlineStyleBlock,
) -> Vec<DeclarationEntrySerialization> {
    crate::declaration_parser::inline_style_declarations_with_importance(block)
        .into_iter()
        .map(|(name, value, important)| DeclarationEntrySerialization {
            name,
            value: crate::value_serialization::ResolvedValueSerialization::new(value),
            important,
        })
        .collect()
}
