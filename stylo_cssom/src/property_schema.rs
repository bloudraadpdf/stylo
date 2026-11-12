use style::properties::PropertyId;
use stylo_cssom_model::PropertySchemaRow;

pub fn property_schema_for_id(property: &PropertyId) -> Option<&'static PropertySchemaRow> {
    match property {
        PropertyId::NonCustom(property) => {
            stylo_cssom_model::STANDARD_PROPERTIES.get(property.unaliased().bit())
        },
        PropertyId::Custom(_) => None,
    }
}

pub fn property_schema(name: &str) -> Option<&'static PropertySchemaRow> {
    let property = PropertyId::parse_enabled_for_all_content(name).ok()?;
    property_schema_for_id(&property)
}

#[test]
fn vertical_align_schema_matches_the_native_longhand() {
    let row = property_schema("vertical-align").expect("native vertical alignment has a schema");
    assert_eq!(row.kind, stylo_cssom_model::PropertyKind::Longhand);
    assert_eq!(row.initial.serialized, "baseline");
    assert!(row.shorthand_expansion.is_empty());
    assert_eq!(row.numeric_grammar, stylo_cssom_model::TypedOmNumericPropertyGrammar::PropertyGrammar);
}

#[test]
fn positions_do_not_admit_scalar_typed_om_numeric_values() {
    use stylo_cssom_model::{TypedOmNumericPropertyGrammar, TypedOmSpecifiedNumericReification};

    crate::context::initialise_required_servo_style_prefs();
    // Typed OM Level 1 has no CSSPositionValue; these retain property-associated values.
    for name in ["perspective-origin", "object-position", "offset-position", "offset-anchor", "transform-origin", "background-position"] {
        let row = property_schema(name).expect("position property has a native schema");
        assert_eq!(row.numeric_grammar, TypedOmNumericPropertyGrammar::Unsupported, "{name}");
        if row.kind == stylo_cssom_model::PropertyKind::Longhand {
            assert_eq!(row.specified_numeric_reification, Some(TypedOmSpecifiedNumericReification::Direct), "{name}");
        }
    }
    for name in ["background-position-x", "background-position-y", "width"] {
        assert_eq!(property_schema(name).unwrap().numeric_grammar,
            TypedOmNumericPropertyGrammar::PropertyGrammar, "{name}");
    }
}

#[test]
fn page_size_descriptor_keeps_its_domain_separate_from_the_element_shorthand() {
    use stylo_cssom_model::{AdapterRoute, PropertyKind};

    let id = PropertyId::NonCustom(style::properties::LonghandId::Size.into());
    let descriptor = property_schema_for_id(&id).expect("page size has a native descriptor row");
    assert_eq!(descriptor.kind, PropertyKind::PageDescriptor);
    assert_eq!(descriptor.parser, AdapterRoute::Stylo);
    assert_eq!(descriptor.serializer, AdapterRoute::Stylo);
    assert_eq!(descriptor.multiplicity, None);
    assert_eq!(descriptor.specified_numeric_reification, None);
    assert_eq!(descriptor.computed_reification, None);

    let shorthand = property_schema("size").expect("element size has its own shorthand row");
    assert_eq!(shorthand.kind, PropertyKind::Shorthand);
    assert_ne!(shorthand.id, descriptor.id);
    assert!(shorthand.multiplicity.is_some());
    assert!(shorthand.specified_numeric_reification.is_some());
}

#[test]
fn webkit_text_fill_color_has_computed_color_reification() {
    let row = property_schema("-webkit-text-fill-color").expect("supported longhand has a schema");
    assert_eq!(
        row.computed_reification,
        Some(stylo_cssom_model::ComputedStyleReificationClass::Color)
    );
}

#[test]
fn webkit_text_stroke_retains_native_longhand_expansion() {
    let row = property_schema("-webkit-text-stroke").unwrap();
    assert_eq!(row.shorthand_expansion, &["-webkit-text-stroke-width", "-webkit-text-stroke-color"]);
    for name in row.shorthand_expansion {
        let longhand = property_schema(name).unwrap();
        assert_eq!(longhand.name, *name);
        assert_eq!(longhand.inheritedness, stylo_cssom_model::Inheritedness::Inherited);
    }
}

#[test]
fn wrap_inside_is_a_reset_standard_longhand() {
    let row = property_schema("wrap-inside").expect("wrap-inside is a supported longhand");
    assert_eq!(row.inheritedness, stylo_cssom_model::Inheritedness::Reset);
    assert_eq!(row.initial.serialized, "auto");
    assert!(property_schema("-bd-wrap-inside").is_none());
}

#[test]
fn typed_declaration_adapter_has_exhaustive_native_ownership() {
    let source = include_str!("rule_parser.rs");
    let adapter = source
        .split_once("fn reconstruct_pinned_declaration")
        .unwrap()
        .1
        .split_once("fn pinned_css_rule_variant")
        .unwrap()
        .0;
    assert!(!adapter.contains("_ =>"));
}

#[test]
fn native_properties_resolve_their_matching_schema() {
    use style::properties::property_counts;
    use stylo_cssom_model::STANDARD_PROPERTIES;

    assert_eq!(STANDARD_PROPERTIES.len(), property_counts::LONGHANDS_AND_SHORTHANDS);
    let native = style::properties::LonghandId::ALL
        .iter()
        .map(|id| (PropertyId::NonCustom((*id).into()), id.name()))
        .chain(
            style::properties::ShorthandId::ALL
                .iter()
                .map(|id| (PropertyId::NonCustom((*id).into()), id.name())),
        );
    for (row, (property, native_name)) in STANDARD_PROPERTIES.iter().zip(native) {
        let resolved = property_schema_for_id(&property).expect("native property has a schema");
        assert_eq!(resolved.name, native_name, "native property ID must resolve its own schema");
        assert_eq!(resolved.name, row.name);
        assert_eq!(resolved.id, row.id);
        if row.kind != stylo_cssom_model::PropertyKind::PageDescriptor
            && let Ok(reparsed) = PropertyId::parse_enabled_for_all_content(row.name)
        {
            assert_eq!(property_schema_for_id(&reparsed).map(|schema| schema.id), Some(row.id));
        }
    }
}

#[test]
fn schema_rows_follow_the_native_property_order() {
    use style::properties::{LonghandId, ShorthandId};
    use stylo_cssom_model::STANDARD_PROPERTIES;

    let native = LonghandId::ALL
        .iter()
        .map(LonghandId::name)
        .chain(ShorthandId::ALL.iter().map(ShorthandId::name));
    for (row, name) in STANDARD_PROPERTIES.iter().zip(native) {
        assert_eq!(row.name, name);
    }
}

#[test]
fn all_schema_retains_native_expansion_order() {
    let native: Vec<_> = style::properties::ShorthandId::All.longhands()
        .map(|id| id.name()).collect();
    let schema = stylo_cssom_model::property_schema("all").unwrap();
    let enabled: Vec<_> = schema.shorthand_expansion.iter().copied()
        .filter(|name| native.contains(name)).collect();
    assert_eq!(enabled, native);
}

#[test]
fn named_scroll_timeline_schema_preserves_vector_longhands_and_shorthand() {
    let row = property_schema("scroll-timeline").expect("enabled shorthand has a schema");
    assert_eq!(row.shorthand_expansion, &["scroll-timeline-name", "scroll-timeline-axis"]);
    for (name, initial) in [("scroll-timeline-name", "none"), ("scroll-timeline-axis", "block")] {
        let row = property_schema(name).expect("enabled longhand has a schema");
        assert_eq!(row.name, name);
        assert_eq!(row.initial.serialized, initial);
        assert_eq!(row.multiplicity, Some(stylo_cssom_model::TypedOmPropertyMultiplicity::CommaSeparated));
    }
}

#[test]
fn initial_metadata_matches_the_native_serializer() {
    use style::properties::{ComputedValues, LonghandId, PropertyDeclarationId, style_structs::Font};
    let initial = ComputedValues::initial_values_with_font_override(Font::initial_values());
    for longhand in [
        LonghandId::XMathScriptMinSize,
        LonghandId::BdPageRegistrationMarkSize,
        LonghandId::NavDown,
        LonghandId::NavLeft,
        LonghandId::NavRight,
        LonghandId::NavUp,
    ] {
        let row = property_schema_for_id(&PropertyId::NonCustom(longhand.into())).unwrap();
        assert_eq!(
            row.initial.serialized,
            initial.computed_value_to_string(PropertyDeclarationId::Longhand(longhand)),
            "{}", row.name,
        );
    }
}
