//! Typed compatibility declarations retained beside the public inline declaration model.

use super::{
    CssomDeclarationPriority, DeclarationPropertyInput, InlineStyleBlock, PropertyDeclaration,
    inline_style_remove_property, inline_style_set_property, parse_inline_style_block, restore_native_declarations,
    retained_native_declarations,
};
use selectors::matching::QuirksMode;
use stylo_cssom_model::{InlineStyleProjection, RetainedNativeDeclarations};

pub fn parse(css: &str, quirks: QuirksMode) -> Option<RetainedNativeDeclarations> {
    retained_native_declarations(&parse_inline_style_block(css, quirks).0)
}

pub fn from_projection(projection: &InlineStyleProjection, quirks: QuirksMode) -> Option<RetainedNativeDeclarations> {
    if projection.is_hydrated() {
        projection.native_backing().cloned()
    } else {
        projection.presentation().and_then(|css| parse(css, quirks))
    }
}

fn block(backing: Option<&RetainedNativeDeclarations>) -> InlineStyleBlock {
    let mut block = InlineStyleBlock(Default::default());
    restore_native_declarations(&mut block.0, backing);
    block
}

pub fn set_property(
    backing: Option<&RetainedNativeDeclarations>,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    quirks: QuirksMode,
) -> Option<RetainedNativeDeclarations> {
    let mut block = block(backing);
    inline_style_set_property(
        &mut block,
        DeclarationPropertyInput::new(property, value),
        priority,
        quirks,
    );
    retained_native_declarations(&block.0)
}

pub fn remove_property(
    backing: Option<&RetainedNativeDeclarations>,
    property: &str,
) -> Option<RetainedNativeDeclarations> {
    let mut block = block(backing);
    inline_style_remove_property(&mut block, property);
    retained_native_declarations(&block.0)
}

pub fn merge(
    backing: Option<&RetainedNativeDeclarations>,
    css: &str,
    quirks: QuirksMode,
) -> Option<RetainedNativeDeclarations> {
    let mut block = block(backing);
    let updates = parse_inline_style_block(css, quirks);
    for (declaration, importance) in updates.0.declaration_importance_iter() {
        if super::is_internal_declaration(declaration) {
            block.0.push(declaration.clone(), importance);
        }
    }
    retained_native_declarations(&block.0)
}

pub fn with_declaration(
    backing: Option<&RetainedNativeDeclarations>,
    declaration: &PropertyDeclaration,
) -> Option<RetainedNativeDeclarations> {
    let mut block = block(backing);
    if super::is_internal_declaration(declaration) {
        block.0.push(declaration.clone(), style::properties::Importance::Normal);
    }
    retained_native_declarations(&block.0)
}

pub fn restore(block: &mut InlineStyleBlock, backing: Option<&RetainedNativeDeclarations>) {
    restore_native_declarations(&mut block.0, backing);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_value(backing: Option<&RetainedNativeDeclarations>) -> String {
        let block = block(backing);
        let mut css = String::new();
        if let Some(declaration) = block.0.declarations().first() {
            if let PropertyDeclaration::WithVariables(value) = declaration {
                return value.value.variable_value().css.clone();
            }
            declaration.to_css(&mut css).expect("native value serializes");
        }
        css
    }

    #[test]
    fn inline_native_alignment_survives_public_mutations_and_clears_with_shorthand() {
        let quirks = QuirksMode::NoQuirks;
        for value in ["middle", "inherit", "var(--alignment)"] {
            let backing = parse(&format!("vertical-align:{value}"), quirks);
            assert_eq!(native_value(backing.as_ref()), value);
            let colour = set_property(
                backing.as_ref(),
                "color",
                "red",
                CssomDeclarationPriority::Normal,
                quirks,
            );
            assert_eq!(colour, backing);
            let modern = set_property(
                colour.as_ref(),
                "alignment-baseline",
                "central",
                CssomDeclarationPriority::Normal,
                quirks,
            );
            assert_eq!(modern, backing);
            assert!(remove_property(modern.as_ref(), "vertical-align").is_none());
        }
        let important = parse("vertical-align:middle!important", quirks);
        assert_eq!(merge(important.as_ref(), "vertical-align:top", quirks), important);
        let replaced = set_property(
            important.as_ref(),
            "vertical-align",
            "top",
            CssomDeclarationPriority::Normal,
            quirks,
        );
        assert_eq!(native_value(replaced.as_ref()), "top");
    }
}
