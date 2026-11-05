/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Scroll-state container features:
//!
//! https://drafts.csswg.org/css-conditional-5/#scroll-state-container

use super::feature::{AllowsRanges, FeatureFlags, QueryFeatureDescription};
use crate::derives::*;
use crate::logical_geometry::{LogicalSide, PhysicalSide, WritingMode};
use crate::values::computed::Context;

bitflags! {
    /// A set of physical edges of a scroll-state container.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct PhysicalEdges: u8 {
        /// The top edge.
        const TOP = 1 << 0;
        /// The right edge.
        const RIGHT = 1 << 1;
        /// The bottom edge.
        const BOTTOM = 1 << 2;
        /// The left edge.
        const LEFT = 1 << 3;
        /// The edges of the horizontal axis.
        const X = Self::LEFT.bits() | Self::RIGHT.bits();
        /// The edges of the vertical axis.
        const Y = Self::TOP.bits() | Self::BOTTOM.bits();
    }
}

impl PhysicalEdges {
    fn side(side: PhysicalSide) -> Self {
        match side {
            PhysicalSide::Top => Self::TOP,
            PhysicalSide::Right => Self::RIGHT,
            PhysicalSide::Bottom => Self::BOTTOM,
            PhysicalSide::Left => Self::LEFT,
        }
    }

    fn logical(side: LogicalSide, wm: WritingMode) -> Self {
        Self::side(side.to_physical(wm))
    }

    fn axis(start: LogicalSide, end: LogicalSide, wm: WritingMode) -> Self {
        Self::logical(start, wm) | Self::logical(end, wm)
    }

    fn block(wm: WritingMode) -> Self {
        Self::axis(LogicalSide::BlockStart, LogicalSide::BlockEnd, wm)
    }

    fn inline(wm: WritingMode) -> Self {
        Self::axis(LogicalSide::InlineStart, LogicalSide::InlineEnd, wm)
    }
}

/// The scroll state of a query container, snapshotted after layout.
///
/// https://drafts.csswg.org/css-conditional-5/#updating-scroll-state
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContainerScrollState {
    /// The edges a sticky positioned container is shifted to stay inside, or `None` when the
    /// container is not sticky positioned.
    pub stuck: Option<PhysicalEdges>,
    /// The axes, as `X` and `Y` edges, in which the container is a snapped snap target.
    pub snapped: PhysicalEdges,
    /// The edges past which a scroll container has reachable scrollable overflow, or `None` when
    /// the container is not a scroll container.
    pub scrollable: Option<PhysicalEdges>,
    /// The directions of the most recent relative scroll, or `None` when the container is not a
    /// scroll container.
    pub scrolled: Option<PhysicalEdges>,
}

/// https://drafts.csswg.org/css-conditional-5/#descdef-container-stuck
#[derive(Clone, Copy, Debug, FromPrimitive, Parse, ToCss)]
#[repr(u8)]
#[allow(missing_docs)]
enum Stuck {
    None,
    Top,
    Right,
    Bottom,
    Left,
    BlockStart,
    InlineStart,
    BlockEnd,
    InlineEnd,
}

/// https://drafts.csswg.org/css-conditional-5/#descdef-container-snapped
#[derive(Clone, Copy, Debug, FromPrimitive, Parse, ToCss)]
#[repr(u8)]
#[allow(missing_docs)]
enum Snapped {
    None,
    X,
    Y,
    Block,
    Inline,
    Both,
}

/// https://drafts.csswg.org/css-conditional-5/#descdef-container-scrollable
#[derive(Clone, Copy, Debug, FromPrimitive, Parse, ToCss)]
#[repr(u8)]
#[allow(missing_docs)]
enum Direction {
    None,
    Top,
    Right,
    Bottom,
    Left,
    BlockStart,
    InlineStart,
    BlockEnd,
    InlineEnd,
    X,
    Y,
    Block,
    Inline,
}

impl Direction {
    fn edges(self, wm: WritingMode) -> PhysicalEdges {
        match self {
            Self::None => PhysicalEdges::empty(),
            Self::Top => PhysicalEdges::TOP,
            Self::Right => PhysicalEdges::RIGHT,
            Self::Bottom => PhysicalEdges::BOTTOM,
            Self::Left => PhysicalEdges::LEFT,
            Self::BlockStart => PhysicalEdges::logical(LogicalSide::BlockStart, wm),
            Self::InlineStart => PhysicalEdges::logical(LogicalSide::InlineStart, wm),
            Self::BlockEnd => PhysicalEdges::logical(LogicalSide::BlockEnd, wm),
            Self::InlineEnd => PhysicalEdges::logical(LogicalSide::InlineEnd, wm),
            Self::X => PhysicalEdges::X,
            Self::Y => PhysicalEdges::Y,
            Self::Block => PhysicalEdges::block(wm),
            Self::Inline => PhysicalEdges::inline(wm),
        }
    }
}

/// Matches `edges` against a value: in the boolean context, any edge matches; `none` matches
/// no edge; any other value matches when it shares an edge with `edges`.
fn match_edges(edges: PhysicalEdges, value: Option<PhysicalEdges>) -> bool {
    match value {
        None => !edges.is_empty(),
        Some(value) if value.is_empty() => edges.is_empty(),
        Some(value) => edges.intersects(value),
    }
}

fn state(context: &Context) -> Option<(ContainerScrollState, WritingMode)> {
    context
        .container_info
        .as_ref()
        .map(|info| (info.scroll_state(), info.writing_mode()))
}

fn eval_stuck(context: &Context, value: Option<Stuck>) -> bool {
    let Some((state, wm)) = state(context) else {
        return false;
    };
    let Some(stuck) = state.stuck else {
        return false;
    };
    let value = value.map(|value| {
        let direction = match value {
            Stuck::None => Direction::None,
            Stuck::Top => Direction::Top,
            Stuck::Right => Direction::Right,
            Stuck::Bottom => Direction::Bottom,
            Stuck::Left => Direction::Left,
            Stuck::BlockStart => Direction::BlockStart,
            Stuck::InlineStart => Direction::InlineStart,
            Stuck::BlockEnd => Direction::BlockEnd,
            Stuck::InlineEnd => Direction::InlineEnd,
        };
        direction.edges(wm)
    });
    match_edges(stuck, value)
}

fn eval_snapped(context: &Context, value: Option<Snapped>) -> bool {
    let Some((state, wm)) = state(context) else {
        return false;
    };
    let axis = |edges: PhysicalEdges| state.snapped.intersects(edges);
    match value {
        None => !state.snapped.is_empty(),
        Some(Snapped::None) => state.snapped.is_empty(),
        Some(Snapped::X) => axis(PhysicalEdges::X),
        Some(Snapped::Y) => axis(PhysicalEdges::Y),
        Some(Snapped::Block) => axis(PhysicalEdges::block(wm)),
        Some(Snapped::Inline) => axis(PhysicalEdges::inline(wm)),
        Some(Snapped::Both) => axis(PhysicalEdges::X) && axis(PhysicalEdges::Y),
    }
}

fn eval_directions(
    context: &Context,
    value: Option<Direction>,
    edges: fn(&ContainerScrollState) -> Option<PhysicalEdges>,
) -> bool {
    let Some((state, wm)) = state(context) else {
        return false;
    };
    edges(&state).is_some_and(|edges| match_edges(edges, value.map(|v| v.edges(wm))))
}

fn eval_scrollable(context: &Context, value: Option<Direction>) -> bool {
    eval_directions(context, value, |state| state.scrollable)
}

fn eval_scrolled(context: &Context, value: Option<Direction>) -> bool {
    eval_directions(context, value, |state| state.scrolled)
}

/// https://drafts.csswg.org/css-conditional-5/#scroll-state-container
pub static SCROLL_STATE_FEATURES: [QueryFeatureDescription; 4] = [
    feature!(
        atom!("stuck"),
        AllowsRanges::No,
        keyword_evaluator!(eval_stuck, Stuck),
        FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE,
    ),
    feature!(
        atom!("snapped"),
        AllowsRanges::No,
        keyword_evaluator!(eval_snapped, Snapped),
        FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE,
    ),
    feature!(
        atom!("scrollable"),
        AllowsRanges::No,
        keyword_evaluator!(eval_scrollable, Direction),
        FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE,
    ),
    feature!(
        atom!("scrolled"),
        AllowsRanges::No,
        keyword_evaluator!(eval_scrolled, Direction),
        FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE,
    ),
];
