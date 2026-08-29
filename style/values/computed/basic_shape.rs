/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS handling for the computed value of
//! [`basic-shape`][basic-shape]s
//!
//! [basic-shape]: https://drafts.csswg.org/css-shapes/#typedef-basic-shape

use crate::values::animated::{Animate, Procedure};
use crate::values::computed::angle::Angle;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Image, LengthPercentage, NonNegativeLength, Position};
use crate::values::generics::basic_shape as generic;
use crate::values::generics::basic_shape::ShapePosition;
use crate::values::generics::border::{GenericBorderCornerRadius, GenericBorderRadius};
use crate::values::generics::position::GenericPositionOrAuto;
use crate::values::generics::NonNegative;
use crate::values::specified::svg_path::{CoordPair, PathCommand};
use crate::values::CSSFloat;

/// A computed alias for FillRule.
pub use crate::values::generics::basic_shape::FillRule;

/// A computed `clip-path` value.
pub type ClipPath = generic::GenericClipPath<BasicShape, ComputedUrl>;

/// A computed `border-shape` value.
pub type BorderShape = generic::BorderShape<BasicShape>;

/// A computed path in `border-shape`.
pub type BorderShapePath = generic::BorderShapePath<BasicShape>;

/// A computed `shape-outside` value.
pub type ShapeOutside = generic::GenericShapeOutside<BasicShape, Image>;

/// A computed `object-view-box` value.
pub type ObjectViewBox = generic::GenericObjectViewBox<InsetRect>;

/// A computed basic shape.
pub type BasicShape =
    generic::GenericBasicShape<Angle, Position, NonNegativeLength, LengthPercentage, InsetRect>;

/// The computed value of `inset()`.
pub type InsetRect = generic::GenericInsetRect<LengthPercentage>;

/// A computed circle.
pub type Circle = generic::Circle<Position, LengthPercentage>;

/// A computed ellipse.
pub type Ellipse = generic::Ellipse<Position, LengthPercentage>;

/// The computed value of `ShapeRadius`.
pub type ShapeRadius = generic::GenericShapeRadius<LengthPercentage>;

/// The computed value of `shape()`.
pub type Shape = generic::Shape<Angle, Position, LengthPercentage>;

/// The computed value of `ShapeCommand`.
pub type ShapeCommand = generic::GenericShapeCommand<Angle, Position, LengthPercentage>;

/// The computed value of `PathOrShapeFunction`.
pub type PathOrShapeFunction =
    generic::GenericPathOrShapeFunction<Angle, Position, LengthPercentage>;

/// The computed value of `CoordinatePair`.
pub type CoordinatePair = generic::CoordinatePair<LengthPercentage>;

/// The computed value of 'ControlPoint'.
pub type ControlPoint = generic::ControlPoint<Position, LengthPercentage>;

/// The computed value of 'RelativeControlPoint'.
pub type RelativeControlPoint = generic::RelativeControlPoint<LengthPercentage>;

/// The computed value of 'CommandEndPoint'.
pub type CommandEndPoint = generic::CommandEndPoint<Position, LengthPercentage>;

/// The computed value of hline and vline's endpoint.
pub type AxisEndPoint = generic::AxisEndPoint<LengthPercentage>;

fn animate_shape_length(
    from: &LengthPercentage,
    to: &LengthPercentage,
    procedure: Procedure,
) -> Result<LengthPercentage, ()> {
    from.animate_as_percentage_dimension_mix(to, procedure)
}

fn animate_shape_position(
    from: &Position,
    to: &Position,
    procedure: Procedure,
) -> Result<Position, ()> {
    Ok(Position::new(
        animate_shape_length(&from.horizontal, &to.horizontal, procedure)?,
        animate_shape_length(&from.vertical, &to.vertical, procedure)?,
    ))
}

fn animate_shape_position_or_auto(
    from: &GenericPositionOrAuto<Position>,
    to: &GenericPositionOrAuto<Position>,
    procedure: Procedure,
) -> Result<GenericPositionOrAuto<Position>, ()> {
    match (from, to) {
        (GenericPositionOrAuto::Position(from), GenericPositionOrAuto::Position(to)) => {
            animate_shape_position(from, to, procedure).map(GenericPositionOrAuto::Position)
        },
        (GenericPositionOrAuto::Auto, GenericPositionOrAuto::Auto) => {
            Ok(GenericPositionOrAuto::Auto)
        },
        (GenericPositionOrAuto::Position(_), GenericPositionOrAuto::Auto)
        | (GenericPositionOrAuto::Auto, GenericPositionOrAuto::Position(_)) => Err(()),
    }
}

fn animate_shape_radius(
    from: &ShapeRadius,
    to: &ShapeRadius,
    procedure: Procedure,
) -> Result<ShapeRadius, ()> {
    match (from, to) {
        (ShapeRadius::Length(from), ShapeRadius::Length(to)) => {
            animate_shape_length(&from.0, &to.0, procedure)
                .map(NonNegative)
                .map(ShapeRadius::Length)
        },
        (ShapeRadius::Length(_), ShapeRadius::ClosestSide | ShapeRadius::FarthestSide)
        | (ShapeRadius::ClosestSide | ShapeRadius::FarthestSide, ShapeRadius::Length(_))
        | (ShapeRadius::ClosestSide, ShapeRadius::ClosestSide)
        | (ShapeRadius::FarthestSide, ShapeRadius::FarthestSide)
        | (ShapeRadius::ClosestSide, ShapeRadius::FarthestSide)
        | (ShapeRadius::FarthestSide, ShapeRadius::ClosestSide) => Err(()),
    }
}

fn animate_shape_corner(
    from: &GenericBorderCornerRadius<NonNegative<LengthPercentage>>,
    to: &GenericBorderCornerRadius<NonNegative<LengthPercentage>>,
    procedure: Procedure,
) -> Result<GenericBorderCornerRadius<NonNegative<LengthPercentage>>, ()> {
    Ok(GenericBorderCornerRadius::new(
        NonNegative(animate_shape_length(
            &from.0.width.0,
            &to.0.width.0,
            procedure,
        )?),
        NonNegative(animate_shape_length(
            &from.0.height.0,
            &to.0.height.0,
            procedure,
        )?),
    ))
}

fn animate_shape_border_radius(
    from: &GenericBorderRadius<NonNegative<LengthPercentage>>,
    to: &GenericBorderRadius<NonNegative<LengthPercentage>>,
    procedure: Procedure,
) -> Result<GenericBorderRadius<NonNegative<LengthPercentage>>, ()> {
    Ok(GenericBorderRadius::new(
        animate_shape_corner(&from.top_left, &to.top_left, procedure)?,
        animate_shape_corner(&from.top_right, &to.top_right, procedure)?,
        animate_shape_corner(&from.bottom_right, &to.bottom_right, procedure)?,
        animate_shape_corner(&from.bottom_left, &to.bottom_left, procedure)?,
    ))
}

fn animate_inset_rect(
    from: &InsetRect,
    to: &InsetRect,
    procedure: Procedure,
) -> Result<InsetRect, ()> {
    use crate::values::generics::rect::Rect;

    Ok(InsetRect {
        rect: Rect::new(
            animate_shape_length(&from.rect.0, &to.rect.0, procedure)?,
            animate_shape_length(&from.rect.1, &to.rect.1, procedure)?,
            animate_shape_length(&from.rect.2, &to.rect.2, procedure)?,
            animate_shape_length(&from.rect.3, &to.rect.3, procedure)?,
        ),
        round: animate_shape_border_radius(&from.round, &to.round, procedure)?,
    })
}

fn animate_polygon(
    from: &generic::Polygon<NonNegativeLength, LengthPercentage>,
    to: &generic::Polygon<NonNegativeLength, LengthPercentage>,
    procedure: Procedure,
) -> Result<generic::Polygon<NonNegativeLength, LengthPercentage>, ()> {
    if from.fill != to.fill || from.coordinates.len() != to.coordinates.len() {
        return Err(());
    }
    let coordinates = from
        .coordinates
        .iter()
        .zip(to.coordinates.iter())
        .map(|(from, to)| {
            Ok(generic::PolygonCoord(
                animate_shape_length(&from.0, &to.0, procedure)?,
                animate_shape_length(&from.1, &to.1, procedure)?,
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    Ok(generic::Polygon {
        fill: from.fill,
        round: from.round.animate(&to.round, procedure)?,
        coordinates: coordinates.into(),
    })
}

impl Animate for BasicShape {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match (self, other) {
            (Self::Rect(from), Self::Rect(to)) => {
                animate_inset_rect(from, to, procedure).map(Self::Rect)
            },
            (Self::Circle(from), Self::Circle(to)) => Ok(Self::Circle(Circle {
                position: animate_shape_position_or_auto(&from.position, &to.position, procedure)?,
                radius: animate_shape_radius(&from.radius, &to.radius, procedure)?,
            })),
            (Self::Ellipse(from), Self::Ellipse(to)) => Ok(Self::Ellipse(Ellipse {
                position: animate_shape_position_or_auto(&from.position, &to.position, procedure)?,
                semiaxis_x: animate_shape_radius(&from.semiaxis_x, &to.semiaxis_x, procedure)?,
                semiaxis_y: animate_shape_radius(&from.semiaxis_y, &to.semiaxis_y, procedure)?,
            })),
            (Self::Polygon(from), Self::Polygon(to)) => {
                animate_polygon(from, to, procedure).map(Self::Polygon)
            },
            (Self::PathOrShape(from), Self::PathOrShape(to)) => {
                from.animate(to, procedure).map(Self::PathOrShape)
            },
            (
                Self::Rect(_),
                Self::Circle(_) | Self::Ellipse(_) | Self::Polygon(_) | Self::PathOrShape(_),
            )
            | (
                Self::Circle(_),
                Self::Rect(_) | Self::Ellipse(_) | Self::Polygon(_) | Self::PathOrShape(_),
            )
            | (
                Self::Ellipse(_),
                Self::Rect(_) | Self::Circle(_) | Self::Polygon(_) | Self::PathOrShape(_),
            )
            | (
                Self::Polygon(_),
                Self::Rect(_) | Self::Circle(_) | Self::Ellipse(_) | Self::PathOrShape(_),
            )
            | (
                Self::PathOrShape(_),
                Self::Rect(_) | Self::Circle(_) | Self::Ellipse(_) | Self::Polygon(_),
            ) => Err(()),
        }
    }
}

/// Animate from `Shape` to `Path`, and vice versa.
macro_rules! animate_shape {
    (
        $from:ident,
        $to:ident,
        $procedure:ident,
        $from_as_shape:tt,
        $to_as_shape:tt
    ) => {{
        // Check fill-rule.
        if $from.fill != $to.fill {
            return Err(());
        }

        // Check the list of commands. (This is a specialized lists::by_computed_value::animate().)
        let from_cmds = $from.commands();
        let to_cmds = $to.commands();
        if from_cmds.len() != to_cmds.len() {
            return Err(());
        }
        let commands = from_cmds
            .iter()
            .zip(to_cmds.iter())
            .map(|(from_cmd, to_cmd)| {
                $from_as_shape(from_cmd).animate(&$to_as_shape(to_cmd), $procedure)
            })
            .collect::<Result<Vec<ShapeCommand>, ()>>()?;

        Ok(Shape {
            fill: $from.fill,
            commands: commands.into(),
        })
    }};
}

impl Animate for PathOrShapeFunction {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        // Per spec, commands are "the same" if they use the same command keyword, and use the same
        // <by-to> keyword. For curve and smooth, they also must have the same number of control
        // points. Therefore, we don't have to do normalization here. (Note that we do
        // normalization if we animate from path() to path(). See svg_path.rs for more details.)
        //
        // https://drafts.csswg.org/css-shapes-2/#interpolating-shape
        match (self, other) {
            (Self::Path(ref from), Self::Path(ref to)) => {
                from.animate(to, procedure).map(Self::Path)
            },
            (Self::Shape(ref from), Self::Shape(ref to)) => {
                from.animate(to, procedure).map(Self::Shape)
            },
            (Self::Shape(ref from), Self::Path(ref to)) => {
                // Animate from shape() to path(). We convert each PathCommand into ShapeCommand,
                // and return shape().
                animate_shape!(
                    from,
                    to,
                    procedure,
                    (|shape_cmd| shape_cmd),
                    (|path_cmd| ShapeCommand::from(path_cmd))
                )
                .map(Self::Shape)
            },
            (Self::Path(ref from), Self::Shape(ref to)) => {
                // Animate from path() to shape(). We convert each PathCommand into ShapeCommand,
                // and return shape().
                animate_shape!(
                    from,
                    to,
                    procedure,
                    (|path_cmd| ShapeCommand::from(path_cmd)),
                    (|shape_cmd| shape_cmd)
                )
                .map(Self::Shape)
            },
        }
    }
}

impl From<&PathCommand> for ShapeCommand {
    #[inline]
    fn from(path: &PathCommand) -> Self {
        match path {
            &PathCommand::Close => Self::Close,
            &PathCommand::Move { ref point } => Self::Move {
                point: point.into(),
            },
            &PathCommand::Line { ref point } => Self::Line {
                point: point.into(),
            },
            &PathCommand::HLine { ref x } => Self::HLine { x: x.into() },
            &PathCommand::VLine { ref y } => Self::VLine { y: y.into() },
            &PathCommand::CubicCurve {
                ref point,
                ref control1,
                ref control2,
            } => Self::CubicCurve {
                point: point.into(),
                control1: control1.into(),
                control2: control2.into(),
            },
            &PathCommand::QuadCurve {
                ref point,
                ref control1,
            } => Self::QuadCurve {
                point: point.into(),
                control1: control1.into(),
            },
            &PathCommand::SmoothCubic {
                ref point,
                ref control2,
            } => Self::SmoothCubic {
                point: point.into(),
                control2: control2.into(),
            },
            &PathCommand::SmoothQuad { ref point } => Self::SmoothQuad {
                point: point.into(),
            },
            &PathCommand::Arc {
                ref point,
                ref radii,
                arc_sweep,
                arc_size,
                rotate,
            } => Self::Arc {
                point: point.into(),
                radii: radii.into(),
                arc_sweep,
                arc_size,
                rotate: Angle::from_degrees(rotate),
            },
        }
    }
}

impl From<&CoordPair> for CoordinatePair {
    #[inline]
    fn from(p: &CoordPair) -> Self {
        use crate::values::computed::CSSPixelLength;
        Self::new(
            LengthPercentage::new_length(CSSPixelLength::new(p.x)),
            LengthPercentage::new_length(CSSPixelLength::new(p.y)),
        )
    }
}

impl From<&ShapePosition<CSSFloat>> for Position {
    #[inline]
    fn from(p: &ShapePosition<CSSFloat>) -> Self {
        use crate::values::computed::CSSPixelLength;
        Self::new(
            LengthPercentage::new_length(CSSPixelLength::new(p.horizontal)),
            LengthPercentage::new_length(CSSPixelLength::new(p.vertical)),
        )
    }
}

impl From<&generic::CommandEndPoint<ShapePosition<CSSFloat>, CSSFloat>> for CommandEndPoint {
    #[inline]
    fn from(p: &generic::CommandEndPoint<ShapePosition<CSSFloat>, CSSFloat>) -> Self {
        match p {
            generic::CommandEndPoint::ToPosition(pos) => Self::ToPosition(pos.into()),
            generic::CommandEndPoint::ByCoordinate(coord) => Self::ByCoordinate(coord.into()),
        }
    }
}

impl From<&generic::AxisEndPoint<CSSFloat>> for AxisEndPoint {
    #[inline]
    fn from(p: &generic::AxisEndPoint<CSSFloat>) -> Self {
        use crate::values::computed::CSSPixelLength;
        use generic::AxisPosition;
        match p {
            generic::AxisEndPoint::ToPosition(AxisPosition::LengthPercent(lp)) => Self::ToPosition(
                AxisPosition::LengthPercent(LengthPercentage::new_length(CSSPixelLength::new(*lp))),
            ),
            generic::AxisEndPoint::ToPosition(AxisPosition::Keyword(_)) => {
                unreachable!("Invalid state: SVG path commands cannot contain a keyword.")
            },
            generic::AxisEndPoint::ByCoordinate(pos) => {
                Self::ByCoordinate(LengthPercentage::new_length(CSSPixelLength::new(*pos)))
            },
        }
    }
}

impl From<&generic::ControlPoint<ShapePosition<CSSFloat>, CSSFloat>> for ControlPoint {
    #[inline]
    fn from(p: &generic::ControlPoint<ShapePosition<CSSFloat>, CSSFloat>) -> Self {
        match p {
            generic::ControlPoint::Absolute(pos) => Self::Absolute(pos.into()),
            generic::ControlPoint::Relative(point) => Self::Relative(RelativeControlPoint {
                coord: CoordinatePair::from(&point.coord),
                reference: point.reference,
            }),
        }
    }
}

impl From<&generic::ArcRadii<CSSFloat>> for generic::ArcRadii<LengthPercentage> {
    #[inline]
    fn from(p: &generic::ArcRadii<CSSFloat>) -> Self {
        use crate::values::computed::CSSPixelLength;
        Self {
            rx: LengthPercentage::new_length(CSSPixelLength::new(p.rx)),
            ry: p
                .ry
                .map(|v| LengthPercentage::new_length(CSSPixelLength::new(v))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BasicShape, Circle, ShapeCommand, ShapeRadius};
    use crate::values::animated::{Animate, Procedure};
    use crate::values::computed::{Length, LengthPercentage, NonNegativeLength, Percentage};
    use crate::values::generics::basic_shape::{FillRule, Polygon, PolygonCoord};
    use crate::values::generics::{position::GenericPositionOrAuto, NonNegative};
    use crate::values::specified::svg_path::{CoordPair, PathCommand};
    use style_traits::ToCss;

    #[test]
    fn path_command_line_preserves_line_variant() {
        let path = PathCommand::Line {
            point: CoordPair::new(20.0, 10.0).into(),
        };

        assert!(matches!(
            ShapeCommand::from(&path),
            ShapeCommand::Line { .. }
        ));
    }

    #[test]
    fn mixed_circle_radius_retains_calculated_representation_at_endpoint() {
        let circle = |radius| {
            BasicShape::Circle(Circle {
                position: GenericPositionOrAuto::Auto,
                radius: ShapeRadius::Length(NonNegative(radius)),
            })
        };
        let from = circle(LengthPercentage::new_length(Length::new(150.0)));
        let to = circle(LengthPercentage::new_percent(Percentage(0.5)));

        let sampled = from
            .animate(&to, Procedure::Interpolate { progress: 0.0 })
            .expect("matching circles must interpolate");

        assert_eq!(sampled.to_css_string(), "circle(calc(0% + 150px))");
    }

    #[test]
    fn keyword_circle_radius_keeps_the_whole_shape_discrete() {
        let circle = |position, radius| {
            BasicShape::Circle(Circle {
                position: GenericPositionOrAuto::Position(position),
                radius,
            })
        };
        let position = |x| {
            crate::values::computed::Position::new(
                LengthPercentage::new_length(Length::new(x)),
                LengthPercentage::new_percent(Percentage(0.75)),
            )
        };
        let from = circle(position(25.0), ShapeRadius::FarthestSide);
        let to = circle(position(50.0), ShapeRadius::FarthestSide);

        assert!(from
            .animate(&to, Procedure::Interpolate { progress: 0.25 })
            .is_err());
    }

    #[test]
    fn polygon_round_interpolates_with_the_vertex_coordinates() {
        let polygon = |round, coordinate| {
            BasicShape::Polygon(Polygon {
                fill: FillRule::Nonzero,
                round: NonNegativeLength::new(round),
                coordinates: vec![PolygonCoord(
                    LengthPercentage::new_length(Length::new(coordinate)),
                    LengthPercentage::new_length(Length::new(coordinate)),
                )]
                .into(),
            })
        };
        let from = polygon(10.0, 0.0);
        let to = polygon(30.0, 50.0);

        let sampled = from
            .animate(&to, Procedure::Interpolate { progress: 0.3 })
            .expect("matching rounded polygons must interpolate");

        assert_eq!(sampled.to_css_string(), "polygon(round 16px, 15px 15px)");
    }
}
