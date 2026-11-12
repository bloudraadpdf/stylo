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
        block.0.push(declaration.clone(), importance);
    }
    retained_native_declarations(&block.0)
}

pub fn with_declaration(
    backing: Option<&RetainedNativeDeclarations>,
    declaration: &PropertyDeclaration,
) -> Option<RetainedNativeDeclarations> {
    let mut block = block(backing);
    let id = match declaration.id() {
        style::properties::PropertyDeclarationId::Longhand(id) => style::properties::PropertyId::NonCustom(id.into()),
        style::properties::PropertyDeclarationId::Custom(name) => style::properties::PropertyId::Custom(name.clone()),
    };
    if let Some(first) = block.0.first_declaration_to_remove(&id) {
        block.0.remove_property(&id, first);
    }
    block.0.push(declaration.clone(), style::properties::Importance::Normal);
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
        if let Some(declaration) = block
            .0
            .declarations()
            .iter()
            .find(|declaration| super::super::is_internal_declaration(declaration))
        {
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
            assert_eq!(native_value(modern.as_ref()), value);
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

    #[test]
    fn inline_native_alignment_origin_clears_on_same_value_longhand_updates() {
        let quirks = QuirksMode::NoQuirks;
        let has_origin = |backing: Option<&RetainedNativeDeclarations>| {
            block(backing).0.declarations().iter().any(|declaration| {
                matches!(declaration, PropertyDeclaration::AlignmentBaseline(value) if value.from_vertical_align_origin())
            })
        };
        for value in ["middle", "text-top", "text-bottom"] {
            let original = parse(&format!("vertical-align:{value}!important"), quirks);
            assert!(has_origin(original.as_ref()));
            let unrelated = set_property(
                original.as_ref(),
                "color",
                "red",
                CssomDeclarationPriority::Normal,
                quirks,
            );
            assert!(has_origin(unrelated.as_ref()));
            let modern = parse_inline_style_block(&format!("alignment-baseline:{value}"), quirks);
            let declaration = &modern.0.declarations()[0];
            for updated in [
                set_property(
                    original.as_ref(),
                    "alignment-baseline",
                    value,
                    CssomDeclarationPriority::Normal,
                    quirks,
                ),
                merge(
                    original.as_ref(),
                    &format!("alignment-baseline:{value}!important"),
                    quirks,
                ),
                with_declaration(original.as_ref(), declaration),
                set_property(
                    original.as_ref(),
                    "all",
                    "initial",
                    CssomDeclarationPriority::Normal,
                    quirks,
                ),
                merge(original.as_ref(), "all:initial!important", quirks),
            ] {
                assert!(!has_origin(updated.as_ref()), "{value}");
                let mut rebuilt = parse_inline_style_block(&format!("alignment-baseline:{value}"), quirks);
                restore(&mut rebuilt, updated.as_ref());
                assert!(
                    !has_origin(retained_native_declarations(&rebuilt.0).as_ref()),
                    "round trip {value}"
                );
            }
        }
    }
}
