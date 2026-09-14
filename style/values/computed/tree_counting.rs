/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::OnceCell;

#[derive(Default)]
pub(super) struct TreeCounting<'a> {
    resolve: Option<Box<dyn Fn() -> (usize, usize) + 'a>>,
    values: OnceCell<(usize, usize)>,
}

impl<'a> TreeCounting<'a> {
    pub(super) fn new(resolve: impl Fn() -> (usize, usize) + 'a) -> Self {
        Self {
            resolve: Some(Box::new(resolve)),
            values: OnceCell::new(),
        }
    }

    pub(super) fn get(&self) -> (usize, usize) {
        *self
            .values
            .get_or_init(|| self.resolve.as_ref().map_or((0, 0), |resolve| resolve()))
    }
}

#[cfg(test)]
mod tests {
    use super::TreeCounting;
    use std::cell::Cell;

    #[test]
    fn sibling_counts_resolve_only_on_first_use() {
        let calls = Cell::new(0);
        let resolve = || {
            calls.set(calls.get() + 1);
            (2, 3)
        };
        let counting = TreeCounting::new(resolve);
        assert_eq!(calls.get(), 0);
        assert_eq!(counting.get(), (2, 3));
        assert_eq!(calls.get(), 1);
        assert_eq!(counting.get(), (2, 3));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn absent_element_has_zero_tree_counts() {
        assert_eq!(TreeCounting::default().get(), (0, 0));
    }
}
