/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed corner shapes with resolved curvature.

use crate::derives::*;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::corner_shape::SuperellipseCurvature;

/// A corner shape whose curvature has been resolved during the cascade.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(transparent)]
pub struct CornerShape(SuperellipseCurvature);

impl CornerShape {
    /// The initial quarter-ellipse shape.
    pub fn round() -> Self {
        Self(SuperellipseCurvature::from_css_number(1.0))
    }

    /// Constructs a shape from its resolved superellipse parameter.
    pub const fn from_curvature(curvature: SuperellipseCurvature) -> Self {
        Self(curvature)
    }

    /// Returns the resolved superellipse parameter.
    pub const fn curvature(self) -> SuperellipseCurvature {
        self.0
    }
}

impl ToCss for CornerShape {
    fn to_css<W: fmt::Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str("superellipse(")?;
        self.0.to_css(dest)?;
        dest.write_char(')')
    }
}
