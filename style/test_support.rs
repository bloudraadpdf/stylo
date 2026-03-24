/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared test helpers for Servo-only Stylo unit tests.

use std::sync::{Mutex, OnceLock};

/// Serialises tests that mutate global style prefs.
pub(crate) fn pref_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Restores a bool pref to its previous value when the guard is dropped.
pub(crate) struct BoolPrefGuard {
    key: &'static str,
    old: bool,
}

impl BoolPrefGuard {
    /// Set a bool pref for the lifetime of the returned guard.
    pub(crate) fn set(key: &'static str, value: bool) -> Self {
        let old = style_config::get_bool(key);
        style_config::set_bool(key, value);
        Self { key, old }
    }
}

impl Drop for BoolPrefGuard {
    fn drop(&mut self) {
        style_config::set_bool(self.key, self.old);
    }
}
