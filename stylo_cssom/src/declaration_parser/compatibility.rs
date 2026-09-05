use std::sync::{Arc, LazyLock};

use stylo_cssom_model::{
    InlineCompatibilityProperty, PropertySchemaRow, SpecifiedDeclaration, SpecifiedPropertyName,
    SpecifiedShorthandSource, SpecifiedStyleValue, StandardPropertyId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanonicalProperty {
    Native(StandardPropertyId),
}

impl CanonicalProperty {
    pub fn from_name(name: &str) -> Option<Self> {
        super::inline_compatibility_property(name)
            .map(Self::from_compatibility)
            .or_else(|| {
                super::inline_style_cssom_property_schema(name)
                    .map(|schema| Self::Native(schema.id))
            })
    }

    pub fn from_specified(property: &SpecifiedPropertyName) -> Option<Self> {
        match property {
            SpecifiedPropertyName::Standard(property) => Some(Self::Native(*property)),
            SpecifiedPropertyName::Compatibility(property) => {
                Some(Self::from_compatibility(*property))
            },
            SpecifiedPropertyName::Custom(_) => None,
        }
    }

    pub fn from_compatibility(property: InlineCompatibilityProperty) -> Self {
        use InlineCompatibilityProperty as Property;

        let native = match property {
            Property::FlowTolerance => "masonry-slack",
            Property::GridLanesPack => "grid-auto-flow",
            Property::Continue => "continue",
            Property::LegacyTextAlign => "text-align",
            Property::LineClamp => "line-clamp",
            Property::WebkitLineClamp => "-webkit-line-clamp",
            Property::WebkitBoxDisplay => "display",
        };
        Self::Native(
            super::inline_style_cssom_property_schema(native)
                .expect("a native compatibility backing has a canonical schema")
                .id,
        )
    }
}

pub fn is_all(schema: &PropertySchemaRow) -> bool {
    static ALL: LazyLock<StandardPropertyId> = LazyLock::new(|| {
        super::inline_style_cssom_property_schema("all")
            .expect("all has a canonical shorthand schema")
            .id
    });
    schema.id == *ALL
}

pub fn shorthand_members(schema: &'static PropertySchemaRow) -> &'static [CanonicalProperty] {
    static MEMBERS: LazyLock<Box<[Box<[CanonicalProperty]>]>> = LazyLock::new(|| {
        stylo_cssom_model::STANDARD_PROPERTIES
            .iter()
            .map(|schema| {
                let members = schema
                    .shorthand_expansion
                    .iter()
                    .map(|name| {
                        CanonicalProperty::Native(
                            stylo_cssom_model::property_schema(name)
                                .expect("canonical shorthand members have schema rows")
                                .id,
                        )
                    })
                    .collect::<Vec<_>>();
                members.into_boxed_slice()
            })
            .collect()
    });
    &MEMBERS[schema.id.index()]
}

pub fn assign_shorthand_source(
    declarations: &mut [SpecifiedDeclaration],
    shorthand: StandardPropertyId,
    value: &SpecifiedStyleValue,
    mutation: bool,
) {
    let source = if mutation {
        SpecifiedShorthandSource::CssomMutation(shorthand)
    } else {
        SpecifiedShorthandSource::Parsed(shorthand)
    };
    for declaration in declarations {
        declaration.shorthand_source = Some(source);
        declaration.shorthand_value = Some(value.clone());
    }
}

pub fn expand_declaration(
    declaration: SpecifiedDeclaration,
    base_url: &Arc<str>,
    mutation: bool,
) -> Vec<SpecifiedDeclaration> {
    let shorthand = match &declaration.property {
        SpecifiedPropertyName::Compatibility(InlineCompatibilityProperty::LineClamp) => {
            "line-clamp"
        },
        SpecifiedPropertyName::Compatibility(InlineCompatibilityProperty::LegacyTextAlign) => {
            "text-align"
        },
        _ => return vec![declaration],
    };
    let url_data = crate::context::ABOUT_BLANK.clone().into();
    let projected = crate::specified::project_inline_style_declaration(&declaration, &url_data);
    let continuation = projected
        .iter()
        .find(|member| {
            member.name() == crate::webkit_box_orient_rewrite::INTERNAL_CONTINUE_PROPERTY
        })
        .and_then(|member| {
            super::parse_inline_compatibility_declaration(
                "continue",
                member.value(),
                declaration.importance,
                base_url,
            )
        });
    let block = super::stylo_inline_style_block(projected, &url_data);
    let mut members =
        super::specified_declarations_from_inline_style_block(&block, base_url.clone())
            .iter()
            .filter(|member| matches!(member.property, SpecifiedPropertyName::Standard(_)))
            .cloned()
            .collect::<Vec<_>>();
    if let Some(continuation) = continuation {
        for member in &mut members {
            if properties_match(&member.property, &continuation.property) {
                *member = continuation.clone();
            }
        }
    }
    let schema = super::inline_style_cssom_property_schema(shorthand)
        .expect("a native compatibility shorthand has a canonical schema");
    assign_shorthand_source(&mut members, schema.id, &declaration.value, mutation);
    members
}

pub const fn css_wide_keyword_text(keyword: stylo_cssom_model::CssWideKeyword) -> &'static str {
    use stylo_cssom_model::CssWideKeyword;

    match keyword {
        CssWideKeyword::Initial => "initial",
        CssWideKeyword::Inherit => "inherit",
        CssWideKeyword::Unset => "unset",
        CssWideKeyword::Revert => "revert",
        CssWideKeyword::RevertLayer => "revert-layer",
    }
}

pub fn properties_match(first: &SpecifiedPropertyName, second: &SpecifiedPropertyName) -> bool {
    if let (SpecifiedPropertyName::Custom(first), SpecifiedPropertyName::Custom(second)) =
        (first, second)
    {
        Arc::ptr_eq(first, second) || first == second
    } else {
        CanonicalProperty::from_specified(first)
            .zip(CanonicalProperty::from_specified(second))
            .is_some_and(|(first, second)| first == second)
    }
}
