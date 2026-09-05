use style::properties::PropertyId;
use stylo_cssom_model::PropertySchemaRow;

pub fn property_schema_for_id(property: &PropertyId) -> Option<&'static PropertySchemaRow> {
    match property {
        PropertyId::NonCustom(property) => {
            stylo_cssom_model::property_schema_at(property.unaliased().bit())
        },
        PropertyId::Custom(_) => None,
    }
}

pub fn property_schema(name: &str) -> Option<&'static PropertySchemaRow> {
    let property = PropertyId::parse_enabled_for_all_content(name).ok()?;
    property_schema_for_id(&property)
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
