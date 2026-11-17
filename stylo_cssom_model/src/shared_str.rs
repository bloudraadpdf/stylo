use std::{
    borrow::{Borrow, Cow},
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    sync::Arc,
};

/// An immutable view of a shared text buffer. Clones and sub-views share the buffer.
#[derive(Clone)]
pub struct SharedStr {
    buffer: Arc<str>,
    range: Range<usize>,
}

impl SharedStr {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.buffer[self.range.clone()]
    }

    /// The view of `range`, relative to this text.
    ///
    /// # Panics
    ///
    /// Panics when `range` is not on `char` boundaries of this text.
    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(
            self.as_str().get(range.clone()).is_some(),
            "a shared text view must lie on char boundaries of its text"
        );
        Self {
            buffer: Arc::clone(&self.buffer),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }

    /// The view of the first `part` at or after byte `from` of this text, else `part` in its own buffer.
    /// A found view moves `from` to its end.
    #[must_use]
    pub fn share(&self, part: &str, from: &mut usize) -> Self {
        let Some(start) = self.as_str()[*from..].find(part).map(|start| *from + start) else {
            return Self::from(part);
        };
        *from = start + part.len();
        self.slice(start..*from)
    }

    #[must_use]
    pub fn same_buffer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.buffer, &other.buffer)
    }

    /// The text from the first view to the last view, when all views are adjacent views of 1 buffer
    /// in order and `ignorable` accepts the text between each 2 views.
    pub fn adjacent_span<'a>(
        views: impl IntoIterator<Item = &'a Self>,
        ignorable: impl Fn(&str) -> bool,
    ) -> Option<&'a str> {
        let mut views = views.into_iter();
        let first = views.next()?;
        let mut end = first.range.end;
        for view in views {
            if !first.same_buffer(view)
                || view.range.start < end
                || !ignorable(&first.buffer[end..view.range.start])
            {
                return None;
            }
            end = view.range.end;
        }
        Some(&first.buffer[first.range.start..end])
    }
}

impl Default for SharedStr {
    fn default() -> Self {
        Self::from("")
    }
}

impl Deref for SharedStr {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for SharedStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SharedStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for SharedStr {
    fn eq(&self, other: &Self) -> bool {
        self.same_buffer(other) && self.range == other.range || self.as_str() == other.as_str()
    }
}

impl Eq for SharedStr {}

impl Hash for SharedStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl fmt::Debug for SharedStr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for SharedStr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<Arc<str>> for SharedStr {
    fn from(buffer: Arc<str>) -> Self {
        let range = 0..buffer.len();
        Self { buffer, range }
    }
}

impl From<&SharedStr> for SharedStr {
    fn from(text: &SharedStr) -> Self {
        text.clone()
    }
}

impl From<&str> for SharedStr {
    fn from(text: &str) -> Self {
        Arc::<str>::from(text).into()
    }
}

impl From<String> for SharedStr {
    fn from(text: String) -> Self {
        Arc::<str>::from(text).into()
    }
}

impl From<&String> for SharedStr {
    fn from(text: &String) -> Self {
        text.as_str().into()
    }
}

impl From<Box<str>> for SharedStr {
    fn from(text: Box<str>) -> Self {
        Arc::<str>::from(text).into()
    }
}

impl From<Cow<'_, str>> for SharedStr {
    fn from(text: Cow<'_, str>) -> Self {
        match text {
            Cow::Borrowed(text) => text.into(),
            Cow::Owned(text) => text.into(),
        }
    }
}

impl From<&Cow<'_, str>> for SharedStr {
    fn from(text: &Cow<'_, str>) -> Self {
        text.as_ref().into()
    }
}

#[cfg(test)]
mod tests {
    use super::SharedStr;

    #[test]
    fn shared_str_views_share_their_buffer() {
        let text = SharedStr::from("a { color: red }");
        let block = text.slice(4..14);
        let value = block.slice(7..10);
        assert_eq!(&*block, "color: red");
        assert_eq!(&*value, "red");
        assert!(value.same_buffer(&text));
        assert!(value.clone().same_buffer(&text));
        assert_eq!(value, SharedStr::from("red"));
        assert!(!value.same_buffer(&SharedStr::from("red")));
    }

    #[test]
    #[should_panic(expected = "char boundaries")]
    fn shared_str_refuses_a_view_inside_a_char() {
        let _ = SharedStr::from("é").slice(0..1);
    }

    #[test]
    fn shared_str_share_finds_a_part_in_its_parent() {
        let rule = SharedStr::from(r#"a { background: url("data:image/png;base64,AAAA"); }"#);
        let mut from = 0;
        let shared = rule.share(r#"url("data:image/png;base64,AAAA")"#, &mut from);
        assert!(shared.same_buffer(&rule));
        assert_eq!(&*shared, r#"url("data:image/png;base64,AAAA")"#);
        assert_eq!(from, rule.len() - 3);
        let owned = rule.share("background", &mut from);
        assert!(!owned.same_buffer(&rule));
        assert_eq!(&*owned, "background");
        assert_eq!(from, rule.len() - 3);
    }

    #[test]
    fn shared_str_adjacent_span_needs_ordered_views_of_1_buffer() {
        let source = SharedStr::from("a{} /* c */ b{} c{}");
        let [a, b, c] = [source.slice(0..3), source.slice(12..15), source.slice(16..19)];
        let blank = |gap: &str| gap.trim().is_empty() || gap.trim() == "/* c */";
        assert_eq!(SharedStr::adjacent_span([&a, &b, &c], blank), Some("a{} /* c */ b{} c{}"));
        assert_eq!(SharedStr::adjacent_span([&b, &a], blank), None);
        assert_eq!(SharedStr::adjacent_span([&a, &c], |gap| gap.trim().is_empty()), None);
        assert_eq!(SharedStr::adjacent_span([&a, &SharedStr::from("b{}")], blank), None);
        assert_eq!(SharedStr::adjacent_span([], blank), None);
    }
}
