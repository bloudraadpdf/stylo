use std::collections::HashSet;

use stylo_cssom_model::{Importance, SpecifiedDeclaration, SpecifiedPropertyName};

use super::compatibility::{properties_match, shorthand_members, CanonicalProperty};

#[derive(Eq, Hash, PartialEq)]
enum DeclarationKey<'a> {
    Standard(CanonicalProperty),
    Custom(&'a str),
}

impl<'a> From<&'a SpecifiedPropertyName> for DeclarationKey<'a> {
    fn from(property: &'a SpecifiedPropertyName) -> Self {
        match property {
            SpecifiedPropertyName::Standard(property) => {
                Self::Standard(CanonicalProperty::Native(*property))
            },
            SpecifiedPropertyName::Compatibility(property) => {
                Self::Standard(CanonicalProperty::from_compatibility(*property))
            },
            SpecifiedPropertyName::Custom(name) => Self::Custom(name),
        }
    }
}

pub fn apply_updates(
    declarations: &mut Vec<SpecifiedDeclaration>,
    updates: impl IntoIterator<Item = SpecifiedDeclaration>,
    replace_important: bool,
) {
    let updates = updates.into_iter().collect::<Vec<_>>();
    if replace_important {
        let replaced = updates
            .iter()
            .map(|update| DeclarationKey::from(&update.property))
            .collect::<HashSet<_>>();
        declarations
            .retain(|existing| !replaced.contains(&DeclarationKey::from(&existing.property)));
        declarations.extend(updates);
        return;
    }
    for update in updates {
        let conflicts = declarations
            .iter()
            .enumerate()
            .filter_map(|(index, existing)| {
                properties_match(&existing.property, &update.property).then_some(index)
            })
            .collect::<Vec<_>>();
        let blocked_by_important = update.importance != Importance::Important
            && conflicts
                .iter()
                .any(|index| declarations[*index].importance == Importance::Important);
        if blocked_by_important {
            continue;
        }
        let insertion = conflicts.first().copied().unwrap_or(declarations.len());
        declarations.retain(|existing| !properties_match(&existing.property, &update.property));
        declarations.insert(insertion.min(declarations.len()), update);
    }
}

pub fn remove_property(declarations: &mut Vec<SpecifiedDeclaration>, property: &str) {
    if property.starts_with("--") {
        declarations.retain(|declaration| {
            !matches!(&declaration.property, SpecifiedPropertyName::Custom(candidate) if candidate.as_ref() == property)
        });
        return;
    }
    let Some(property) = CanonicalProperty::from_name(property) else {
        return;
    };
    let members = match property {
        CanonicalProperty::Native(property)
            if property.schema().kind == stylo_cssom_model::PropertyKind::Shorthand =>
        {
            shorthand_members(property.schema())
        },
        _ => &[],
    };
    declarations.retain(|declaration| {
        CanonicalProperty::from_specified(&declaration.property)
            .is_none_or(|candidate| candidate != property && !members.contains(&candidate))
    });
}
