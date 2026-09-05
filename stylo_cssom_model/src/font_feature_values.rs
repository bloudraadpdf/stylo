use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontFeatureKind {
    Annotation,
    Ornaments,
    Stylistic,
    Swash,
    CharacterVariant,
    Styleset,
    HistoricalForms,
}

impl FontFeatureKind {
    pub const ALL: [Self; 7] = [
        Self::Annotation,
        Self::Ornaments,
        Self::Stylistic,
        Self::Swash,
        Self::CharacterVariant,
        Self::Styleset,
        Self::HistoricalForms,
    ];

    pub const fn attribute(self) -> &'static str {
        match self {
            Self::Annotation => "annotation",
            Self::Ornaments => "ornaments",
            Self::Stylistic => "stylistic",
            Self::Swash => "swash",
            Self::CharacterVariant => "characterVariant",
            Self::Styleset => "styleset",
            Self::HistoricalForms => "historicalForms",
        }
    }

    pub const fn at_keyword(self) -> &'static str {
        match self {
            Self::CharacterVariant => "character-variant",
            Self::HistoricalForms => "historical-forms",
            _ => self.attribute(),
        }
    }

    fn accepts(self, count: usize) -> bool {
        count > 0
            && match self {
                Self::Styleset => true,
                Self::CharacterVariant => count <= 2,
                _ => count == 1,
            }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidFontFeatureValueCount;

#[derive(Clone, Debug, Eq, PartialEq)]
struct FeatureEntry {
    name: Arc<str>,
    values: Arc<[u32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontFeatureMap {
    kind: FontFeatureKind,
    entries: Vec<Option<FeatureEntry>>,
}

impl FontFeatureMap {
    fn new(kind: FontFeatureKind) -> Self {
        Self {
            kind,
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, name: &str) -> Option<&[u32]> {
        self.entries
            .iter()
            .flatten()
            .find(|entry| entry.name.as_ref() == name)
            .map(|entry| entry.values.as_ref())
    }
    pub fn next_entry(&self, cursor: usize) -> Option<(usize, &str, &[u32])> {
        self.entries
            .iter()
            .enumerate()
            .skip(cursor)
            .find_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|entry| (index + 1, entry.name.as_ref(), entry.values.as_ref()))
            })
    }
    pub fn set(
        &mut self,
        name: impl Into<Arc<str>>,
        values: impl Into<Arc<[u32]>>,
    ) -> Result<(), InvalidFontFeatureValueCount> {
        let values = values.into();
        if !self.kind.accepts(values.len()) {
            return Err(InvalidFontFeatureValueCount);
        }
        let name = name.into();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .flatten()
            .find(|entry| entry.name == name)
        {
            entry.values = values;
        } else {
            self.entries.push(Some(FeatureEntry { name, values }));
        }
        Ok(())
    }
    pub fn delete(&mut self, name: &str) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry
                .as_ref()
                .is_some_and(|entry| entry.name.as_ref() == name)
        }) else {
            return false;
        };
        *entry = None;
        true
    }
    pub fn clear(&mut self) {
        self.entries.fill(None);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleFontFeatureValues {
    font_family: Arc<str>,
    maps: [FontFeatureMap; 7],
}

impl RuleFontFeatureValues {
    pub fn new(font_family: impl Into<Arc<str>>) -> Self {
        Self {
            font_family: font_family.into(),
            maps: FontFeatureKind::ALL.map(FontFeatureMap::new),
        }
    }
    pub fn font_family(&self) -> &str {
        &self.font_family
    }
    pub fn set_font_family(&mut self, font_family: impl Into<Arc<str>>) {
        self.font_family = font_family.into();
    }
    pub fn map(&self, kind: FontFeatureKind) -> &FontFeatureMap {
        &self.maps[kind as usize]
    }
    pub fn map_mut(&mut self, kind: FontFeatureKind) -> &mut FontFeatureMap {
        &mut self.maps[kind as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_maps_preserve_iteration_positions_across_mutations() {
        let mut map = FontFeatureMap::new(FontFeatureKind::Annotation);
        assert!(map.set("invalid", vec![1, 2]).is_err());
        assert!(map.set("invalid", vec![]).is_err());
        map.set("a", vec![1]).unwrap();
        map.set("b", vec![2]).unwrap();
        let cursor = map.next_entry(0).unwrap().0;
        map.delete("a");
        map.set("a", vec![3]).unwrap();
        assert_eq!(map.next_entry(cursor), Some((2, "b", &[2][..])));
        map.clear();
        map.set("c", vec![4]).unwrap();
        assert_eq!(map.next_entry(cursor), Some((4, "c", &[4][..])));
    }
}
