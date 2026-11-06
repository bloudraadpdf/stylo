/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed `calc-size()` values and their interpolation.
//!
//! <https://drafts.csswg.org/css-values-5/#interp-calc-size>

use crate::values::animated::{Animate, Procedure};
use crate::values::computed::length::{MaxSize, NonNegativeLengthPercentage, Size};
use crate::values::computed::length_percentage::{
    anchor_size_calc_node, CalcLengthPercentageLeaf, CalcNode as ComputedCalcNode, ToAnchorCalcNode,
};
use crate::values::computed::LengthPercentage;
use crate::values::generics::NonNegative;
use style_traits::values::specified::AllowedNumericType;
use crate::values::generics::length::{GenericCalcSize, GenericCalcSizeBasis};
use crate::values::specified::calc::{CalcNode, CalcSizeCalculation, Leaf};
use crate::values::specified::length::{AbsoluteLength, NoCalcLength};

/// A computed `calc-size()` value.
pub type CalcSize = GenericCalcSize<CalcSizeCalculation>;

/// A computed `calc-size()` basis.
pub type CalcSizeBasis = GenericCalcSizeBasis<CalcSizeCalculation>;

fn calculation(lp: &NonNegativeLengthPercentage) -> CalcSizeCalculation {
    CalcSizeCalculation::new(lp.0.to_calc_node().map_leaves(|leaf| match *leaf {
        CalcLengthPercentageLeaf::Length(l) => Leaf::Length(NoCalcLength::Absolute(AbsoluteLength::Px(l.px()))),
        CalcLengthPercentageLeaf::Percentage(p) => Leaf::Percentage(p.0),
        CalcLengthPercentageLeaf::Number(n) => Leaf::Number(n),
    }))
}

fn mix(
    basis: &CalcSizeBasis,
    from: &CalcSizeCalculation,
    to: &CalcSizeCalculation,
    procedure: Procedure,
) -> CalcSize {
    let (from_weight, to_weight) = procedure.weights();
    CalcSize {
        basis: basis.clone(),
        calculation: from.weighted_sum(from_weight, to, to_weight),
    }
}

impl CalcSize {
    /// `calc-size(<keyword>, size)`.
    pub fn from_keyword(basis: CalcSizeBasis) -> Self {
        Self {
            basis,
            calculation: CalcSizeCalculation(CalcNode::Leaf(Leaf::Size)),
        }
    }

    /// Flattens nested and `<calc-sum>` bases into a keyword, `any` or `100%`.
    ///
    /// Fails when the flattened calculation would be too large.
    ///
    /// <https://drafts.csswg.org/css-values-5/#prepare-calc-size>
    pub fn prepared(&self) -> Option<Self> {
        let (basis, size) = match &self.basis {
            CalcSizeBasis::CalcSize(inner) => {
                let inner = inner.prepared()?;
                (inner.basis, inner.calculation)
            },
            CalcSizeBasis::Calc(basis) if basis.has_percentage() => (
                CalcSizeBasis::Calc(CalcSizeCalculation(CalcNode::Leaf(Leaf::Percentage(1.)))),
                basis.depercentify(),
            ),
            CalcSizeBasis::Calc(basis) => (CalcSizeBasis::Any, basis.clone()),
            _ => return Some(self.clone()),
        };
        Some(Self {
            basis,
            calculation: self.calculation.substitute_size(&size.0)?,
        })
    }
}

impl Animate for CalcSize {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self.basis == other.basis {
            return Ok(mix(&self.basis, &self.calculation, &other.calculation, procedure));
        }
        let (from, to) = (self.prepared().ok_or(())?, other.prepared().ok_or(())?);
        let basis = match (&from.basis, &to.basis) {
            (a, b) if a == b => a,
            (CalcSizeBasis::Any, basis) | (basis, CalcSizeBasis::Any) => basis,
            _ => return Err(()),
        };
        Ok(mix(basis, &from.calculation, &to.calculation, procedure))
    }
}

/// Implements `calc-size()` interpolation for a sizing property.
macro_rules! animate_size {
    ($ty:ident, $($keyword:ident => $basis:ident),+) => {
        impl $ty {
            fn to_calc_size(&self, allow_keywords: bool) -> Result<CalcSize, ()> {
                match self {
                    Self::CalcSize(value) => Ok((**value).clone()),
                    $(Self::$keyword if allow_keywords => Ok(CalcSize::from_keyword(CalcSizeBasis::$basis)),)+
                    _ => Err(()),
                }
            }

            /// Animates two values, with `interpolate-size: allow-keywords` when
            /// `allow_keywords` is true.
            ///
            /// <https://drafts.csswg.org/css-values-5/#interpolate-size>
            pub fn animate_size(
                &self,
                other: &Self,
                procedure: Procedure,
                allow_keywords: bool,
            ) -> Result<Self, ()> {
                Ok(match (self, other) {
                    (Self::LengthPercentage(a), Self::LengthPercentage(b)) => {
                        Self::LengthPercentage(a.animate(b, procedure)?)
                    },
                    (Self::AnchorSizeFunction(_) | Self::AnchorContainingCalcFunction(_), _)
                    | (_, Self::AnchorSizeFunction(_) | Self::AnchorContainingCalcFunction(_)) => {
                        Self::AnchorContainingCalcFunction(NonNegative(LengthPercentage::mix_anchor_values(
                            self,
                            other,
                            procedure,
                            AllowedNumericType::NonNegative,
                        )?))
                    },
                    (Self::LengthPercentage(lp), _) => {
                        let to = other.to_calc_size(allow_keywords)?;
                        Self::CalcSize(Box::new(mix(&to.basis, &calculation(lp), &to.calculation, procedure)))
                    },
                    (_, Self::LengthPercentage(lp)) => {
                        let from = self.to_calc_size(allow_keywords)?;
                        Self::CalcSize(Box::new(mix(&from.basis, &from.calculation, &calculation(lp), procedure)))
                    },
                    (Self::CalcSize(_), _) | (_, Self::CalcSize(_)) => Self::CalcSize(Box::new(
                        self.to_calc_size(true)?.animate(&other.to_calc_size(true)?, procedure)?,
                    )),
                    _ if self == other => self.clone(),
                    _ => return Err(()),
                })
            }
        }

        impl Animate for $ty {
            fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
                self.animate_size(other, procedure, false)
            }
        }

        impl ToAnchorCalcNode for $ty {
            fn to_anchor_calc_node(&self) -> Result<ComputedCalcNode, ()> {
                match self {
                    Self::LengthPercentage(lp) | Self::AnchorContainingCalcFunction(lp) => {
                        Ok(lp.0.to_calc_node())
                    },
                    Self::AnchorSizeFunction(function) => anchor_size_calc_node(&**function),
                    _ => Err(()),
                }
            }
        }
    };
}

animate_size!(Size,
    Auto => Auto,
    MinContent => MinContent,
    MaxContent => MaxContent,
    FitContent => FitContent,
    Stretch => Stretch
);

animate_size!(MaxSize,
    MinContent => MinContent,
    MaxContent => MaxContent,
    FitContent => FitContent,
    Stretch => Stretch
);
