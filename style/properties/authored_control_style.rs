/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Form-control box facts from winning author declarations and computed values.

use super::{LonghandId, LonghandIdSet, StyleBuilder};
use crate::values::computed::Image;
use crate::values::specified::border::BorderStyle;
use crate::Zero;

/// The winning author contribution to the preferred block dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthoredBlockSize {
    /// No author-origin winner.
    #[default]
    Unspecified,
    /// An author-origin automatic size.
    Auto,
    /// An author-origin non-automatic size.
    NonAuto,
}

/// Authored box components used when laying out and painting form controls.
/// These facts belong to this element, rather than being inherited from its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredControlStyle {
    /// An authored background colour or image can paint.
    pub background: bool,
    /// An authored border width, style or image can establish a frame.
    pub border: bool,
    /// At least one authored padding edge is non-zero.
    pub padding: bool,
    /// A margin edge has an author-origin winner.
    pub margin: bool,
    /// An authored preferred or constrained dimension is not automatic/unbounded.
    pub size: bool,
    /// The author contribution to the preferred block dimension.
    pub block_size: AuthoredBlockSize,
    /// Overflow has an author-origin winner.
    pub overflow: bool,
    /// Text alignment has an author-origin winner on this element.
    pub text_align: bool,
}

impl AuthoredControlStyle {
    pub(crate) fn from_cascade(authored: &LonghandIdSet, style: &StyleBuilder) -> Self {
        let background = style.get_background();
        let border = style.get_border();
        let padding = style.get_padding();
        let position = style.get_position();
        let block_id = if style.writing_mode.is_vertical() {
            LonghandId::Width
        } else {
            LonghandId::Height
        };
        let block_size = if style.writing_mode.is_vertical() {
            &position.width
        } else {
            &position.height
        };
        Self {
            background: (authored.contains(LonghandId::BackgroundColor)
                && background
                    .background_color
                    .as_absolute()
                    .is_none_or(|color| color.alpha > 0.0))
                || (authored.contains(LonghandId::BackgroundImage)
                    && background
                        .background_image
                        .0
                        .iter()
                        .any(|image| !matches!(image, Image::None))),
            border: [
                (
                    LonghandId::BorderTopWidth,
                    &border.border_top_width,
                    LonghandId::BorderTopStyle,
                    border.border_top_style,
                ),
                (
                    LonghandId::BorderRightWidth,
                    &border.border_right_width,
                    LonghandId::BorderRightStyle,
                    border.border_right_style,
                ),
                (
                    LonghandId::BorderBottomWidth,
                    &border.border_bottom_width,
                    LonghandId::BorderBottomStyle,
                    border.border_bottom_style,
                ),
                (
                    LonghandId::BorderLeftWidth,
                    &border.border_left_width,
                    LonghandId::BorderLeftStyle,
                    border.border_left_style,
                ),
            ]
            .iter()
            .any(|(width_id, width, style_id, value)| {
                if authored.contains(*style_id) {
                    !matches!(value, BorderStyle::None | BorderStyle::Hidden)
                } else {
                    authored.contains(*width_id) && width.0 != app_units::Au(0)
                }
            }) || (authored.contains(LonghandId::BorderImageSource)
                && !matches!(border.border_image_source, Image::None)),
            padding: [
                (LonghandId::PaddingTop, &padding.padding_top),
                (LonghandId::PaddingRight, &padding.padding_right),
                (LonghandId::PaddingBottom, &padding.padding_bottom),
                (LonghandId::PaddingLeft, &padding.padding_left),
            ]
            .iter()
            .any(|(id, value)| authored.contains(*id) && !value.is_zero()),
            margin: [
                LonghandId::MarginTop,
                LonghandId::MarginRight,
                LonghandId::MarginBottom,
                LonghandId::MarginLeft,
            ]
            .iter()
            .any(|id| authored.contains(*id)),
            size: [
                (LonghandId::Width, &position.width),
                (LonghandId::Height, &position.height),
                (LonghandId::MinWidth, &position.min_width),
                (LonghandId::MinHeight, &position.min_height),
            ]
            .iter()
            .any(|(id, value)| authored.contains(*id) && !value.is_auto())
                || [
                    (LonghandId::MaxWidth, &position.max_width),
                    (LonghandId::MaxHeight, &position.max_height),
                ]
                .iter()
                .any(|(id, value)| {
                    authored.contains(*id)
                        && !matches!(value, crate::values::computed::MaxSize::None)
                }),
            block_size: if !authored.contains(block_id) {
                AuthoredBlockSize::Unspecified
            } else if block_size.is_auto() {
                AuthoredBlockSize::Auto
            } else {
                AuthoredBlockSize::NonAuto
            },
            overflow: authored.contains(LonghandId::OverflowX)
                || authored.contains(LonghandId::OverflowY),
            text_align: authored.contains(LonghandId::TextAlignAll),
        }
    }
}
