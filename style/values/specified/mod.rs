/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified values.
//!
//! TODO(emilio): Enhance docs.

use super::computed::transform::DirectionVector;
use super::computed::{Context, ToComputedValue};
use super::generics::grid::ImplicitGridTracks as GenericImplicitGridTracks;
use super::generics::grid::{GridLine as GenericGridLine, TrackBreadth as GenericTrackBreadth};
use super::generics::grid::{TrackList as GenericTrackList, TrackSize as GenericTrackSize};
use super::generics::transform::IsParallelTo;
use super::generics::{self, GreaterThanOrEqualToOne, NonNegative};
use super::{CSSFloat, CSSInteger};
use crate::context::QuirksMode;
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::properties_and_values::syntax::Descriptor;
use crate::values::generics::calc::SortKey as AttrUnit;
use crate::values::specified::calc::CalcNode;
use crate::values::{serialize_atom_identifier, serialize_number};
use crate::{Atom, LocalName, Namespace, One, Prefix, Zero};
use cssparser::{match_ignore_ascii_case, Parser, Token};
use std::fmt::{self, Write};
use std::ops::Add;
use style_traits::values::specified::AllowedNumericType;
use style_traits::{
    CssString, CssWriter, NumericValue, ParseError, SpecifiedValueInfo, StyleParseErrorKind, ToCss,
    ToTyped, TypedValue,
};

pub use self::align::{
    AlignTracks, ContentDistribution, ItemPlacement, JustifyItems, JustifyTracks, SelfAlignment,
};
pub use self::angle::{AllowUnitlessZeroAngle, Angle};
pub use self::animation::{
    AnimationComposition, AnimationDirection, AnimationDuration, AnimationFillMode,
    AnimationIterationCount, AnimationName, AnimationPlayState, AnimationTimeline, ScrollAxis,
    TimelineName, TransitionBehavior, TransitionProperty, ViewTimelineInset, ViewTransitionClass,
    ViewTransitionName,
};
pub use self::background::{BackgroundRepeat, BackgroundSize};
pub use self::basic_shape::FillRule;
pub use self::bd_a11y::{
    BdTextReplace, BdTextReplacement, BdTextReplacementMethod, BdTextReplacementPoint, BdTooltip,
};
pub use self::bd_barcode::{
    BdBarcodeAffix, BdBarcodeCheckDigitMode, BdBarcodeCompositeType, BdBarcodeContent,
    BdBarcodeEccLevel, BdBarcodeEncoding, BdBarcodeFontFamily, BdBarcodeHrPosition,
    BdBarcodeReaderInit, BdBarcodeSize, BdBarcodeStructuredAppend, BdBarcodeType, BdQrEccLetter,
};
pub use self::bd_bfo::{BdIndex, BdIndexGrouping};
pub use self::bd_bookmark::{BdPdfLinkType, BookmarkTarget};
pub use self::bd_change_bar::{
    BdChangeBarAlign, BdChangeBarColour, BdChangeBarExclusion, BdChangeBarName, BdChangeBarOffset,
    BdChangeBarWidth,
};
pub use self::bd_color_function::BdColorFunction;
pub use self::bd_filter_resolution::BdFilterResolution;
pub use self::bd_float::{
    BdFloatDeferColumn, BdFloatDeferPage, BdFloatDisplace, BdFloatModifier, BdFloatPolicy,
    BdFloatTail,
};
pub use self::bd_flow::{BdFlowFrom, BdFlowInto, BdFlowIntoMode, BdRegionFragment};
pub use self::bd_footnote::{
    BdFootnoteFragmentation, BdFootnoteRuleLength, FloatPlacement, FootnoteStylePosition,
};
pub use self::bd_gaps::{BorderClip, MaskBorderMode, Overlay};
pub use self::bd_hyphenation::{
    BdHyphenateLimitLines, BdHyphenateLines, BdHyphenatePatterns, BdHyphenateWordLength,
    BdLinebreakMagic,
};
pub use self::bd_image::{
    BdImageClipPath, BdImageInteractivity, BdImageMagic, BdImageOrientation, BdImageRecompression,
    BdImageResampling, BdImageResolution,
};
pub use self::bd_line_grid::{BdBaselineGrid, BdLineGrid, BdLineSnap, BdLineStackingStrategy};
pub use self::bd_link::{BdLink, BdLinkArea};
pub use self::bd_misc::{
    BdCaptionPage, BdColumnClip, BdFlow, BdIntegerAuto, BdLang, BdLineBreakOpportunity,
    BdLineBreakRule, BdObjectSlice, BdPositionOrigin, BdReplacedElement, BdScaleContent,
    BdShrinkToFit, BdTabSnap, BdTargetCandidate, BdTruncateMarginAfterBreak,
};
pub use self::bd_page_boxes::{
    BdPdfArtBox, BdPdfArtSize, BdPdfBleedBox, BdPdfCropBox, BdPdfCropSize, BdPdfMediaSize,
    BdPdfPageBoxInsets, BdPdfPageBoxInsetsSides, BdPdfPageBoxSize, BdPdfPageClip, BdPdfTrimBox,
};
pub use self::bd_page_group::BdPageGroup;
pub use self::bd_page_margin::BdPageMarginEdge;
pub use self::bd_page_marks::{
    BdBleedColour, BdColorBarPosition, BdColourBarPositionSide, BdColourBarSwatches, BdCropColour,
    BdPageMarkEnabled, BdPageMarkLength, BdPageMarkLengthOrAuto, BdPageMarkOffset, BdPageMarkWidth,
    BdPageMarksColour, BdPrintMarkSet, BdRegistrationColour, BdRegistrationPosition,
    BdSidenoteGlyph, BdSidenoteMarkerOffset,
};
pub use self::bd_page_rotation::{BdPdfPageRotation, BdRotateBody};
pub use self::bd_pagination::{
    BdBlankPageContent, BdChangeLineBreaksForPagination, BdForcedBreaks, BdKeepWithPrevious,
    BdLineBreakChoices, BdNLines, BdOrphansFragments, BdPageFill, BdPdfSignature, BdResizeAdjust,
    BdResizeOptions, BdSpreadLengthOptions, BdTextWrap, BdWrapInside,
};
pub use self::bd_pdf::BdPdfMetaValue;
pub use self::bd_pdf_colour::{BdPdfOverprint, BdPdfOverprintContent, BdPdfPageColourSpace};
pub use self::bd_pdf_comment::{
    BdPdfCommentAuthor, BdPdfCommentColour, BdPdfCommentDate, BdPdfCommentDateFormat,
    BdPdfCommentIcon, BdPdfCommentKind, BdPdfCommentOpen, BdPdfCommentPosition, BdPdfCommentState,
    BdPdfCommentStateModel, BdPdfCommentString, BdPdfCommentSubject, BdPdfLinkArea,
    BdPdfLinkBorder, BdPdfLinkBorderColor, BdPdfLinkBorderStyle, BdPdfLinkBorderWidth,
};
pub use self::bd_pdf_conformance::{BdPdfConformanceValue, BdPdfVersionValue};
pub use self::bd_pdf_custom_property::{BdPdfCustomProperty, BdPdfCustomPropertyEntry};
pub use self::bd_pdf_destination::{
    BdDestinationArea, BdPdfAttachmentIcon, BdPdfAttachmentLocation,
    BdPdfAttachmentModificationDate, BdPdfAttachmentOrder, BdPdfAttachmentRelationship,
    BdPdfAttachmentUrl, BdPdfStringSlot,
};
pub use self::bd_pdf_form::{
    BdPdfAnnotationHidden, BdPdfFormFieldFlags, BdPdfFormFieldMaxLength, BdPdfFormFieldMkCaption,
    BdPdfFormFieldMkColour, BdPdfFormFieldMkIcon, BdPdfFormFieldMkIconFit,
    BdPdfFormFieldMkIconFitScaleType, BdPdfFormFieldMkIconFitScaleWhen,
    BdPdfFormFieldMkIconFitValue, BdPdfFormFieldMkRotation, BdPdfFormFieldMkTextPosition,
    BdPdfSignatureFieldLock, BdPdfSignatureFieldLockFields, BdPdfSignatureFieldName,
};
pub use self::bd_pdf_format::BdPdfFormat;
pub use self::bd_pdf_layer::{BdPdfLayer, BdPdfLayerIntent, BdPdfLayerVisible};
pub use self::bd_pdf_multimedia::{BdPdfMultimedia, BdPdfMultimediaFormat};
pub use self::bd_pdf_output::{
    BdFontEmbeddingType, BdGlyphLayoutMode, BdPaintReordering, BdPdfBookmarksEnabled,
    BdPdfPassdownStyles, BdPdfRasterAccessibility, BdPdfShapeOptimization, BdPdfTextRendering,
    BdRasterization, BdRasterizationMaxSize, BdRasterizationSupersampling,
};
pub use self::bd_pdf_output_condition::BdPdfOutputCondition;
pub use self::bd_pdf_output_intent::{
    BdPdfColourConversion, BdPdfColourOption, BdPdfColourOptions, BdPdfFallbackCmykProfile,
    BdPdfOutputIntent,
};
pub use self::bd_pdf_output_registry_name::BdPdfOutputRegistryName;
pub use self::bd_pdf_role_map::{BdPdfRoleMap, BdPdfRoleMapEntry};
pub use self::bd_pdf_script::{
    BdPdfEventKind, BdPdfEventScript, BdPdfEventScripts, BdPdfOpenActionScript, BdPdfScript,
    BdPdfWidgetActionScript,
};
pub use self::bd_pdf_stamp::{BdPdfStampIcon, BdPdfStampIntent, BdPdfStampString};
pub use self::bd_pdf_tag::{
    BdPdfArtifactKind, BdPdfStandardRole, BdPdfTagForm, BdPdfTagFormChecked, BdPdfTagFormName,
    BdPdfTagHeaderCellScope, BdPdfTagNamespace, BdPdfTagStringAuto, BdPdfTagStringPlain,
    BdPdfTagTableSummary, BdPdfTagValue,
};
pub use self::bd_pdf_tagged::BdPdfTagged;
pub use self::bd_pdf_trapped::BdPdfTrapped;
pub use self::bd_pdf_viewer::{
    BdFirstPageSide, BdInitialPage, BdInitialZoom, BdPagesCounterOffset, BdPdfTriState,
    BdPdfViewerDirection, BdPdfViewerDuplex, BdPdfViewerNonFullscreenPageMode,
    BdPdfViewerNumCopies, BdPdfViewerPageBox, BdPdfViewerPageLayout, BdPdfViewerPageMode,
    BdPdfViewerPrintPageRange, BdPdfViewerPrintScaling,
};
pub use self::bd_running_copy::BdRunningCopy;
pub use self::bd_sidenote::{
    BdFloatReferenceSidenote, BdSidenoteAlign, BdSidenoteAlignment, BdSidenoteAvoid,
    BdSidenoteOffset, BdSidenoteSide,
};
pub use self::bd_source::{BdSource, BdSourceArea, BdSourcePage};
pub use self::bd_tab_stops::{BdTabStop, BdTabStopAlignment, BdTabStopLeader, BdTabStops};
pub use self::bd_text_decoration::{
    BdTextDecorationLineColour, BdTextDecorationLineStyle, BdTextDecorationLineThickness,
    BdTextDecorationSkip, BdTextDecorationSkipCategory, BdTextDecorationTrim, BdTextEmphasisSkip,
    BdTextUnderlineOffset, BdTextUnderlinePosition,
};
pub use self::border::{
    BorderCornerRadius, BorderImageRepeat, BorderImageSideWidth, BorderImageSlice,
    BorderImageWidth, BorderRadius, BorderSideOffset, BorderSideWidth, BorderSpacing, BorderStyle,
    LineWidth,
};
pub use self::box_::{
    AlignmentBaseline, Appearance, BaselineShift, BaselineSource, BookmarkLevel, BookmarkState,
    BreakBetween, BreakWithin, Clear, Contain, ContainIntrinsicSize, ContainerName, ContainerType,
    ContentVisibility, Display, Float, FloatDefer, FloatOffset, FloatReference, FootnoteDisplay,
    FootnotePolicy, LineClamp, MarginBreak, MarginTrim, Overflow, OverflowAnchor,
    OverflowClipMargin, OverscrollBehavior, Perspective, PositionProperty, Resize, ScrollSnapAlign,
    ScrollSnapAxis, ScrollSnapStop, ScrollSnapStrictness, ScrollSnapType, ScrollbarGutter,
    TouchAction, WillChange, WillChangeBits, WritingModeProperty, Zoom,
};
pub use self::color::{
    Color, ColorOrAuto, ColorPropertyValue, ColorScheme, ForcedColorAdjust, PrintColorAdjust,
};
pub use self::color_5::{OutputColorModel, PredefinedOutputColourSpace};
pub use self::color_hdr_1::DynamicRangeLimit;
pub use self::column::ColumnCount;
pub use self::corner_shape::{CornerShape, CornerShapeRect};
pub use self::counters::{
    BookmarkLabel, Content, ContentItem, CounterIncrement, CounterReset, CounterSet, StringSet,
};
pub use self::display_4::ReadingFlow;
pub use self::easing::TimingFunction;
pub use self::effects::{BoxShadow, Filter, SimpleShadow};
pub use self::exclusions_1::{WrapFlow, WrapThrough};
pub use self::flex::FlexBasis;
pub use self::font::{FontFamily, FontLanguageOverride, FontPalette, FontStyle};
pub use self::font::{FontFeatureSettings, FontVariantLigatures, FontVariantNumeric};
pub use self::font::{
    FontSize, FontSizeAdjust, FontSizeAdjustFactor, FontSizeKeyword, FontStretch, FontSynthesis,
    FontSynthesisStyle,
};
pub use self::font::{FontVariantAlternates, FontWeight};
pub use self::font::{FontVariantEastAsian, FontVariationSettings, LineHeight};
pub use self::font::{MathDepth, MozScriptMinSize, MozScriptSizeMultiplier, XLang, XTextScale};
pub use self::image::{EndingShape as GradientEndingShape, Gradient, Image, ImageRendering};
pub use self::length::{AbsoluteLength, CalcLengthPercentage, CharacterWidth};
pub use self::length::{FontRelativeLength, Length, LengthOrNumber, NonNegativeLengthOrNumber};
pub use self::length::{LengthOrAuto, LengthPercentage, LengthPercentageOrAuto};
pub use self::length::{Margin, MaxSize, Size};
pub use self::length::{NoCalcLength, ViewportPercentageLength, ViewportVariant};
pub use self::length::{
    NonNegativeLength, NonNegativeLengthPercentage, NonNegativeLengthPercentageOrAuto,
};
pub use self::line_grid_1::{BoxSnap, LineGrid, LineSnap};
pub use self::list::ListStyleType;
pub use self::list::Quotes;
pub use self::motion::{OffsetPath, OffsetPosition, OffsetRotate};
pub use self::outline::OutlineStyle;
pub use self::overflow_4::{BlockEllipsis, Continue, LeadingTrim, MaxLines, PositiveLineCount};
pub use self::page::{
    Bleed, PageMarks, PageName, PageOrientation, PageSize, PageSizeOrientation, PaperSize,
    PrinceBleed, PrinceBleedSides,
};
pub use self::percentage::{NonNegativePercentage, Percentage};
pub use self::position::AnchorFunction;
pub use self::position::AnchorName;
pub use self::position::AnchorNameIdent;
pub use self::position::AnchorScope;
pub use self::position::AnchorScopeKeyword;
pub use self::position::AspectRatio;
pub use self::position::Inset;
pub use self::position::PositionAnchor;
pub use self::position::PositionAnchorKeyword;
pub use self::position::PositionTryFallbacks;
pub use self::position::PositionTryOrder;
pub use self::position::PositionVisibility;
pub use self::position::{GridAutoFlow, GridTemplateAreas, Position, PositionOrAuto};
pub use self::position::{MasonryAutoFlow, MasonryItemOrder, MasonryPlacement, MasonrySlack};
pub use self::position::{PositionArea, PositionAreaKeyword};
pub use self::position::{PositionComponent, ZIndex};
pub use self::ratio::Ratio;
pub use self::rect::NonNegativeLengthOrNumberRect;
pub use self::regions_1::{FlowFrom, FlowInto, FlowIntoMode, RegionFragment};
pub use self::resolution::Resolution;
pub use self::rhythm_1::{BlockStepAlign, BlockStepInsert, BlockStepRound, BlockStepSize};
pub use self::ruby_1::{RubyMerge, RubyOverhang};
pub use self::sizing_4::MinIntrinsicSizing;
pub use self::svg::{DProperty, MozContextProperties};
pub use self::svg::{SVGLength, SVGOpacity, SVGPaint};
pub use self::svg::{SVGPaintOrder, SVGStrokeDashArray, SVGWidth, VectorEffect};
pub use self::svg_path::SVGPathData;
pub use self::text::RubyPosition;
pub use self::text::{
    HangingPunctuation, InitialLetter, LetterSpacing, LineBreak, TextAlign, TextCombineUpright,
    TextIndent,
};
pub use self::text::{HyphenateCharacter, HyphenateLimitChars};
pub use self::text::{
    OverflowWrap, TextEmphasisPosition, TextEmphasisStyle, WordBreak, WordSpaceTransform,
};
pub use self::text::{TextAlignKeyword, TextDecorationLine, TextOverflow, WordSpacing};
pub use self::text::{TextAlignLast, TextAutospace, TextUnderlinePosition};
pub use self::text::{
    TextDecorationInset, TextDecorationLength, TextDecorationSkipInk, TextJustify, TextTransform,
};
pub use self::text_decor_4::{TextDecorationSkipKind, TextDecorationTrim, TextEmphasisSkip};
pub use self::time::Time;
pub use self::transform::{Rotate, Scale, Transform};
pub use self::transform::{TransformBox, TransformOrigin, TransformStyle, Translate};
#[cfg(feature = "gecko")]
pub use self::ui::CursorImage;
pub use self::ui::{
    BoolInteger, Cursor, Inert, MozTheme, PointerEvents, ScrollbarColor, UserFocus, UserSelect,
};
pub use super::generics::grid::GridTemplateComponent as GenericGridTemplateComponent;

pub mod align;
pub mod angle;
pub mod animation;
pub mod background;
pub mod basic_shape;
pub mod bd_a11y;
pub mod bd_barcode;
pub mod bd_bfo;
pub mod bd_bookmark;
pub mod bd_change_bar;
pub mod bd_color_function;
pub mod bd_filter_resolution;
pub mod bd_float;
pub mod bd_flow;
pub mod bd_footnote;
pub mod bd_gaps;
pub mod bd_hyphenation;
pub mod bd_image;
pub mod bd_line_grid;
pub mod bd_link;
pub mod bd_misc;
pub mod bd_page_boxes;
pub mod bd_page_group;
pub mod bd_page_margin;
pub mod bd_page_marks;
pub mod bd_page_rotation;
pub mod bd_pagination;
pub mod bd_pdf;
pub mod bd_pdf_colour;
pub mod bd_pdf_comment;
pub mod bd_pdf_conformance;
pub mod bd_pdf_custom_property;
pub mod bd_pdf_destination;
pub mod bd_pdf_form;
pub mod bd_pdf_format;
pub mod bd_pdf_layer;
pub mod bd_pdf_multimedia;
pub mod bd_pdf_output;
pub mod bd_pdf_output_condition;
pub mod bd_pdf_output_intent;
pub mod bd_pdf_output_registry_name;
pub mod bd_pdf_role_map;
pub mod bd_pdf_script;
pub mod bd_pdf_stamp;
pub mod bd_pdf_tag;
pub mod bd_pdf_tagged;
pub mod bd_pdf_trapped;
pub mod bd_pdf_viewer;
pub mod bd_running_copy;
pub mod bd_sidenote;
pub mod bd_source;
pub mod bd_tab_stops;
pub mod bd_text_decoration;
pub mod border;
#[path = "box.rs"]
pub mod box_;
pub mod calc;
pub mod color;
pub mod color_5;
pub mod color_hdr_1;
pub mod column;
pub mod corner_shape;
pub mod counters;
pub mod display_4;
pub mod easing;
pub mod effects;
pub mod exclusions_1;
pub mod flex;
pub mod font;
pub mod grid;
pub mod image;
pub mod intersection_observer;
pub mod length;
pub mod line_grid_1;
pub mod list;
pub mod motion;
pub mod outline;
pub mod overflow_4;
pub mod page;
pub mod percentage;
pub mod position;
pub mod ratio;
pub mod rect;
pub mod regions_1;
pub mod resolution;
pub mod rhythm_1;
pub mod ruby_1;
pub mod sizing_4;
pub mod source_size_list;
pub mod svg;
pub mod svg_path;
pub mod table;
pub mod text;
pub mod text_decor_4;
pub mod time;
pub mod transform;
pub mod ui;
pub mod url;

/// <angle> | <percentage>
/// https://drafts.csswg.org/css-values/#typedef-angle-percentage
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem)]
pub enum AngleOrPercentage {
    Percentage(Percentage),
    Angle(Angle),
}

impl AngleOrPercentage {
    fn parse_internal<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allow_unitless_zero: AllowUnitlessZeroAngle,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(per) = input.try_parse(|i| Percentage::parse(context, i)) {
            return Ok(AngleOrPercentage::Percentage(per));
        }

        Angle::parse_internal(context, input, allow_unitless_zero).map(AngleOrPercentage::Angle)
    }

    /// Allow unitless angles, used for conic-gradients as specified by the spec.
    /// https://drafts.csswg.org/css-images-4/#valdef-conic-gradient-angle
    pub fn parse_with_unitless<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        AngleOrPercentage::parse_internal(context, input, AllowUnitlessZeroAngle::Yes)
    }
}

impl Parse for AngleOrPercentage {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        AngleOrPercentage::parse_internal(context, input, AllowUnitlessZeroAngle::No)
    }
}

/// Parse a `<number>` value, with a given clamping mode.
fn parse_number_with_clamping_mode<'i, 't>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
    clamping_mode: AllowedNumericType,
) -> Result<Number, ParseError<'i>> {
    let location = input.current_source_location();
    match *input.next()? {
        Token::Number { value, .. } if clamping_mode.is_ok(context.parsing_mode, value) => {
            Ok(Number {
                value,
                calc_clamping_mode: None,
            })
        },
        Token::Function(ref name) => {
            let function = CalcNode::math_function(context, name, location)?;
            let value = CalcNode::parse_number(context, input, function)?;
            Ok(Number {
                value,
                calc_clamping_mode: Some(clamping_mode),
            })
        },
        ref t => Err(location.new_unexpected_token_error(t.clone())),
    }
}

/// A CSS `<number>` specified value.
///
/// https://drafts.csswg.org/css-values-3/#number-value
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialOrd, ToShmem)]
pub struct Number {
    /// The numeric value itself.
    value: CSSFloat,
    /// If this number came from a calc() expression, this tells how clamping
    /// should be done on the value.
    calc_clamping_mode: Option<AllowedNumericType>,
}

impl Parse for Number {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        parse_number_with_clamping_mode(context, input, AllowedNumericType::All)
    }
}

impl PartialEq<Number> for Number {
    fn eq(&self, other: &Number) -> bool {
        if self.calc_clamping_mode != other.calc_clamping_mode {
            return false;
        }

        self.value == other.value || (self.value.is_nan() && other.value.is_nan())
    }
}

impl Number {
    /// Returns a new number with the value `val`.
    #[inline]
    fn new_with_clamping_mode(
        value: CSSFloat,
        calc_clamping_mode: Option<AllowedNumericType>,
    ) -> Self {
        Self {
            value,
            calc_clamping_mode,
        }
    }

    /// Returns this percentage as a number.
    pub fn to_percentage(&self) -> Percentage {
        Percentage::new_with_clamping_mode(self.value, self.calc_clamping_mode)
    }

    /// Returns a new number with the value `val`.
    #[inline]
    pub fn new(val: CSSFloat) -> Self {
        Self::new_with_clamping_mode(val, None)
    }

    /// Returns whether this number came from a `calc()` expression.
    #[inline]
    pub fn was_calc(&self) -> bool {
        self.calc_clamping_mode.is_some()
    }

    /// Returns the numeric value, clamped if needed.
    #[inline]
    pub fn get(&self) -> f32 {
        crate::values::normalize(
            self.calc_clamping_mode
                .map_or(self.value, |mode| mode.clamp(self.value)),
        )
        .min(f32::MAX)
        .max(f32::MIN)
    }

    #[allow(missing_docs)]
    pub fn parse_non_negative<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Number, ParseError<'i>> {
        parse_number_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
    }

    #[allow(missing_docs)]
    pub fn parse_at_least_one<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Number, ParseError<'i>> {
        parse_number_with_clamping_mode(context, input, AllowedNumericType::AtLeastOne)
    }

    /// Clamp to 1.0 if the value is over 1.0.
    #[inline]
    pub fn clamp_to_one(self) -> Self {
        Number {
            value: self.value.min(1.),
            calc_clamping_mode: self.calc_clamping_mode,
        }
    }
}

impl ToComputedValue for Number {
    type ComputedValue = CSSFloat;

    #[inline]
    fn to_computed_value(&self, _: &Context) -> CSSFloat {
        self.get()
    }

    #[inline]
    fn from_computed_value(computed: &CSSFloat) -> Self {
        Number {
            value: *computed,
            calc_clamping_mode: None,
        }
    }
}

impl ToCss for Number {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_number(self.value, self.calc_clamping_mode.is_some(), dest)
    }
}

impl ToTyped for Number {
    fn to_typed(&self) -> Option<TypedValue> {
        let value = self.value;
        let unit = CssString::from("number");
        Some(TypedValue::Numeric(NumericValue::Unit { value, unit }))
    }
}

impl IsParallelTo for (Number, Number, Number) {
    fn is_parallel_to(&self, vector: &DirectionVector) -> bool {
        use euclid::approxeq::ApproxEq;
        // If a and b is parallel, the angle between them is 0deg, so
        // a x b = |a|*|b|*sin(0)*n = 0 * n, |a x b| == 0.
        let self_vector = DirectionVector::new(self.0.get(), self.1.get(), self.2.get());
        self_vector
            .cross(*vector)
            .square_length()
            .approx_eq(&0.0f32)
    }
}

impl SpecifiedValueInfo for Number {}

impl Add for Number {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.get() + other.get())
    }
}

impl Zero for Number {
    #[inline]
    fn zero() -> Self {
        Self::new(0.)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.get() == 0.
    }
}

impl From<Number> for f32 {
    #[inline]
    fn from(n: Number) -> Self {
        n.get()
    }
}

impl From<Number> for f64 {
    #[inline]
    fn from(n: Number) -> Self {
        n.get() as f64
    }
}

/// A Number which is >= 0.0.
pub type NonNegativeNumber = NonNegative<Number>;

impl Parse for NonNegativeNumber {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        parse_number_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
            .map(NonNegative::<Number>)
    }
}

impl One for NonNegativeNumber {
    #[inline]
    fn one() -> Self {
        NonNegativeNumber::new(1.0)
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.get() == 1.0
    }
}

impl NonNegativeNumber {
    /// Returns a new non-negative number with the value `val`.
    pub fn new(val: CSSFloat) -> Self {
        NonNegative::<Number>(Number::new(val.max(0.)))
    }

    /// Returns the numeric value.
    #[inline]
    pub fn get(&self) -> f32 {
        self.0.get()
    }
}

/// An Integer which is >= 0.
pub type NonNegativeInteger = NonNegative<Integer>;

impl Parse for NonNegativeInteger {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(NonNegative(Integer::parse_non_negative(context, input)?))
    }
}

/// A Number which is >= 1.0.
pub type GreaterThanOrEqualToOneNumber = GreaterThanOrEqualToOne<Number>;

impl Parse for GreaterThanOrEqualToOneNumber {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        parse_number_with_clamping_mode(context, input, AllowedNumericType::AtLeastOne)
            .map(GreaterThanOrEqualToOne::<Number>)
    }
}

/// <number> | <percentage>
///
/// Accepts only non-negative numbers.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem)]
pub enum NumberOrPercentage {
    Percentage(Percentage),
    Number(Number),
}

impl NumberOrPercentage {
    fn parse_with_clamping_mode<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        type_: AllowedNumericType,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(per) =
            input.try_parse(|i| Percentage::parse_with_clamping_mode(context, i, type_))
        {
            return Ok(NumberOrPercentage::Percentage(per));
        }

        parse_number_with_clamping_mode(context, input, type_).map(NumberOrPercentage::Number)
    }

    /// Parse a non-negative number or percentage.
    pub fn parse_non_negative<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
    }

    /// Convert the number or the percentage to a number.
    pub fn to_percentage(self) -> Percentage {
        match self {
            Self::Percentage(p) => p,
            Self::Number(n) => n.to_percentage(),
        }
    }

    /// Convert the number or the percentage to a number.
    pub fn to_number(self) -> Number {
        match self {
            Self::Percentage(p) => p.to_number(),
            Self::Number(n) => n,
        }
    }
}

impl Parse for NumberOrPercentage {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::All)
    }
}

/// A non-negative <number> | <percentage>.
pub type NonNegativeNumberOrPercentage = NonNegative<NumberOrPercentage>;

impl NonNegativeNumberOrPercentage {
    /// Returns the `100%` value.
    #[inline]
    pub fn hundred_percent() -> Self {
        NonNegative(NumberOrPercentage::Percentage(Percentage::hundred()))
    }

    /// Return a particular number.
    #[inline]
    pub fn new_number(n: f32) -> Self {
        NonNegative(NumberOrPercentage::Number(Number::new(n)))
    }
}

impl Parse for NonNegativeNumberOrPercentage {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(NonNegative(NumberOrPercentage::parse_non_negative(
            context, input,
        )?))
    }
}

/// The value of Opacity is <alpha-value>, which is "<number> | <percentage>".
/// However, we serialize the specified value as number, so it's ok to store
/// the Opacity as Number.
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    PartialOrd,
    SpecifiedValueInfo,
    ToCss,
    ToShmem,
    ToTyped,
)]
pub struct Opacity(Number);

impl Parse for Opacity {
    /// Opacity accepts <number> | <percentage>, so we parse it as NumberOrPercentage,
    /// and then convert into an Number if it's a Percentage.
    /// https://drafts.csswg.org/cssom/#serializing-css-values
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let number = NumberOrPercentage::parse(context, input)?.to_number();
        Ok(Opacity(number))
    }
}

impl ToComputedValue for Opacity {
    type ComputedValue = CSSFloat;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> CSSFloat {
        let value = self.0.to_computed_value(context);
        if context.for_smil_animation {
            // SMIL expects to be able to interpolate between out-of-range
            // opacity values.
            value
        } else {
            value.min(1.0).max(0.0)
        }
    }

    #[inline]
    fn from_computed_value(computed: &CSSFloat) -> Self {
        Opacity(Number::from_computed_value(computed))
    }
}

/// A specified `<integer>`, either a simple integer value or a calc expression.
/// Note that a calc expression may not actually be an integer; it will be rounded
/// at computed-value time.
///
/// <https://drafts.csswg.org/css-values/#integers>
#[derive(Clone, Copy, Debug, MallocSizeOf, ToShmem, ToTyped)]
pub struct Integer(IntegerValue);

#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, PartialOrd, ToShmem, ToTyped)]
enum IntegerValue {
    Literal(CSSInteger),
    Calc(CSSFloat),
    NonNegativeCalc(CSSFloat),
    PositiveCalc(CSSFloat),
}

impl Zero for Integer {
    #[inline]
    fn zero() -> Self {
        Self::new(0)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == 0
    }
}

impl One for Integer {
    #[inline]
    fn one() -> Self {
        Self::new(1)
    }

    #[inline]
    fn is_one(&self) -> bool {
        *self == 1
    }
}

impl PartialEq<i32> for Integer {
    fn eq(&self, value: &i32) -> bool {
        self.value() == *value
    }
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value().partial_cmp(&other.value())
    }
}

impl Integer {
    /// Trivially constructs a new `Integer` value.
    pub fn new(val: CSSInteger) -> Self {
        Self(IntegerValue::Literal(val))
    }

    /// Returns the (rounded) integer value associated with this value.
    pub fn value(&self) -> CSSInteger {
        let value = match self.0 {
            IntegerValue::Literal(value) => return value,
            IntegerValue::Calc(value) => value,
            IntegerValue::NonNegativeCalc(value) => value.max(0.0),
            IntegerValue::PositiveCalc(value) => value.max(1.0),
        };
        (value + 0.5).floor() as CSSInteger
    }

    /// Trivially constructs a new integer value from a `calc()` expression.
    fn from_calc(value: CSSFloat, clamping_mode: AllowedNumericType) -> Self {
        let value = match clamping_mode {
            AllowedNumericType::All => IntegerValue::Calc(value),
            AllowedNumericType::NonNegative => IntegerValue::NonNegativeCalc(value),
            AllowedNumericType::AtLeastOne => IntegerValue::PositiveCalc(value),
            AllowedNumericType::ZeroToOne => unreachable!(),
        };
        Self(value)
    }

    fn parse_with_clamping_mode<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        clamping_mode: AllowedNumericType,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        match *input.next()? {
            Token::Number {
                int_value: Some(v), ..
            } if clamping_mode.is_ok(context.parsing_mode, v as CSSFloat) => Ok(Integer::new(v)),
            Token::Function(ref name) => {
                let function = CalcNode::math_function(context, name, location)?;
                let result = CalcNode::parse_number(context, input, function)?;
                Ok(Integer::from_calc(result, clamping_mode))
            },
            ref t => Err(location.new_unexpected_token_error(t.clone())),
        }
    }
}

impl Parse for Integer {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::All)
    }
}

impl Integer {
    /// Parse a non-negative integer.
    pub fn parse_non_negative<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Integer, ParseError<'i>> {
        Integer::parse_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
    }

    /// Parse a positive integer (>= 1).
    pub fn parse_positive<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Integer, ParseError<'i>> {
        Integer::parse_with_clamping_mode(context, input, AllowedNumericType::AtLeastOne)
    }
}

impl ToComputedValue for Integer {
    type ComputedValue = i32;

    #[inline]
    fn to_computed_value(&self, _: &Context) -> i32 {
        self.value()
    }

    #[inline]
    fn from_computed_value(computed: &i32) -> Self {
        Integer::new(*computed)
    }
}

impl ToCss for Integer {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self.0 {
            IntegerValue::Literal(value) => value.to_css(dest),
            IntegerValue::Calc(value)
            | IntegerValue::NonNegativeCalc(value)
            | IntegerValue::PositiveCalc(value) => serialize_number(value, true, dest),
        }
    }
}

impl SpecifiedValueInfo for Integer {}

/// A wrapper of Integer, with value >= 1.
pub type PositiveInteger = GreaterThanOrEqualToOne<Integer>;

impl Parse for PositiveInteger {
    #[inline]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Integer::parse_positive(context, input).map(GreaterThanOrEqualToOne)
    }
}

/// The specified value of a grid `<track-breadth>`
pub type TrackBreadth = GenericTrackBreadth<LengthPercentage>;

/// The specified value of a grid `<track-size>`
pub type TrackSize = GenericTrackSize<LengthPercentage>;

/// The specified value of a grid `<track-size>+`
pub type ImplicitGridTracks = GenericImplicitGridTracks<TrackSize>;

/// The specified value of a grid `<track-list>`
/// (could also be `<auto-track-list>` or `<explicit-track-list>`)
pub type TrackList = GenericTrackList<LengthPercentage, Integer>;

/// The specified value of a `<grid-line>`.
pub type GridLine = GenericGridLine<Integer>;

/// `<grid-template-rows> | <grid-template-columns>`
pub type GridTemplateComponent = GenericGridTemplateComponent<LengthPercentage, Integer>;

/// rect(...)
pub type ClipRect = generics::GenericClipRect<LengthOrAuto>;

impl Parse for ClipRect {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_quirky(context, input, AllowQuirks::No)
    }
}

impl ClipRect {
    /// Parses a rect(<top>, <left>, <bottom>, <right>), allowing quirks.
    fn parse_quirky<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allow_quirks: AllowQuirks,
    ) -> Result<Self, ParseError<'i>> {
        input.expect_function_matching("rect")?;

        fn parse_argument<'i, 't>(
            context: &ParserContext,
            input: &mut Parser<'i, 't>,
            allow_quirks: AllowQuirks,
        ) -> Result<LengthOrAuto, ParseError<'i>> {
            LengthOrAuto::parse_quirky(context, input, allow_quirks)
        }

        input.parse_nested_block(|input| {
            let top = parse_argument(context, input, allow_quirks)?;
            let right;
            let bottom;
            let left;

            if input.try_parse(|input| input.expect_comma()).is_ok() {
                right = parse_argument(context, input, allow_quirks)?;
                input.expect_comma()?;
                bottom = parse_argument(context, input, allow_quirks)?;
                input.expect_comma()?;
                left = parse_argument(context, input, allow_quirks)?;
            } else {
                right = parse_argument(context, input, allow_quirks)?;
                bottom = parse_argument(context, input, allow_quirks)?;
                left = parse_argument(context, input, allow_quirks)?;
            }

            Ok(ClipRect {
                top,
                right,
                bottom,
                left,
            })
        })
    }
}

/// rect(...) | auto
pub type ClipRectOrAuto = generics::GenericClipRectOrAuto<ClipRect>;

impl ClipRectOrAuto {
    /// Parses a ClipRect or Auto, allowing quirks.
    pub fn parse_quirky<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allow_quirks: AllowQuirks,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(v) = input.try_parse(|i| ClipRect::parse_quirky(context, i, allow_quirks)) {
            return Ok(generics::GenericClipRectOrAuto::Rect(v));
        }
        input.expect_ident_matching("auto")?;
        Ok(generics::GenericClipRectOrAuto::Auto)
    }
}

/// Whether quirks are allowed in this context.
#[derive(Clone, Copy, PartialEq)]
pub enum AllowQuirks {
    /// Quirks are not allowed.
    No,
    /// Quirks are allowed, in quirks mode.
    Yes,
    /// Quirks are always allowed, used for SVG lengths.
    Always,
}

impl AllowQuirks {
    /// Returns `true` if quirks are allowed in this context.
    pub fn allowed(self, quirks_mode: QuirksMode) -> bool {
        match self {
            AllowQuirks::Always => true,
            AllowQuirks::No => false,
            AllowQuirks::Yes => quirks_mode == QuirksMode::Quirks,
        }
    }
}

/// An attr(...) rule
///
/// `[namespace? `|`]? ident [attr-type]? [, fallback]?`
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(u8)]
pub enum AttrSyntax {
    /// No explicit attr type.
    #[default]
    None,
    /// `raw-string`
    RawString,
    /// Legacy keyword / unit syntax such as `string`, `url`, `number`, or `px`.
    Keyword(crate::OwnedStr),
    /// `type(<syntax>)`
    Type(crate::OwnedStr),
}

/// The namespace-aware name selected by an `attr()` function.
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(u8)]
pub enum AttrName {
    /// A name whose namespace prefix was absent, empty, or bound.
    Resolved {
        /// The authored namespace prefix, retained for serialization.
        prefix: Prefix,
        /// The expanded namespace URL.
        namespace: Namespace,
        /// The case-sensitive local name.
        local_name: Atom,
    },
    /// A syntactically valid name whose namespace prefix is not bound.
    UnresolvedNamespace {
        /// The unbound authored prefix.
        prefix: Prefix,
        /// The case-sensitive local name.
        local_name: Atom,
    },
}

impl AttrName {
    pub(crate) fn parse_with_namespaces<'i, 't>(
        input: &mut Parser<'i, 't>,
        namespaces: &crate::stylesheets::Namespaces,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let first = match input.next()? {
            Token::Ident(name) => Some(name.clone()),
            Token::Delim('|') => None,
            token => return Err(location.new_unexpected_token_error(token.clone())),
        };

        let Some(prefix) = first else {
            let local_name = match input.next_including_whitespace()? {
                Token::Ident(name) => Atom::from(name.as_ref()),
                token => return Err(location.new_unexpected_token_error(token.clone())),
            };
            return Ok(Self::Resolved {
                prefix: Prefix::default(),
                namespace: Namespace::default(),
                local_name,
            });
        };

        let after_name = input.state();
        if !matches!(input.next_including_whitespace(), Ok(Token::Delim('|'))) {
            input.reset(&after_name);
            return Ok(Self::Resolved {
                prefix: Prefix::default(),
                namespace: Namespace::default(),
                local_name: Atom::from(prefix.as_ref()),
            });
        }

        let local_name = match input.next_including_whitespace()? {
            Token::Ident(name) => Atom::from(name.as_ref()),
            token => return Err(location.new_unexpected_token_error(token.clone())),
        };
        let prefix = Prefix::from(prefix.as_ref());
        Ok(match namespaces.prefixes.get(&prefix) {
            Some(namespace) => Self::Resolved {
                prefix,
                namespace: namespace.clone(),
                local_name,
            },
            None => Self::UnresolvedNamespace { prefix, local_name },
        })
    }

    /// Return the local name independently of namespace resolution.
    pub fn local_name(&self) -> &Atom {
        match self {
            Self::Resolved { local_name, .. } | Self::UnresolvedNamespace { local_name, .. } => {
                local_name
            },
        }
    }

    /// Return the expanded name, or `None` when the prefix is unbound.
    pub fn expanded_name(&self) -> Option<crate::dom::ExpandedAttributeName> {
        match self {
            Self::Resolved {
                namespace,
                local_name,
                ..
            } => Some(crate::dom::ExpandedAttributeName {
                namespace: namespace.clone(),
                local_name: LocalName::from(local_name.as_ref()),
            }),
            Self::UnresolvedNamespace { .. } => None,
        }
    }

    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let (prefix, local_name) = match self {
            Self::Resolved {
                prefix, local_name, ..
            }
            | Self::UnresolvedNamespace {
                prefix, local_name, ..
            } => (prefix, local_name),
        };
        if !prefix.is_empty() {
            serialize_atom_identifier(prefix, dest)?;
            dest.write_char('|')?;
        }
        serialize_atom_identifier(local_name, dest)
    }
}

impl ToCss for AttrSyntax {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::None => Ok(()),
            Self::RawString => dest.write_str("raw-string"),
            Self::Keyword(keyword) => {
                if &**keyword == "%" {
                    dest.write_char('%')
                } else {
                    dest.write_str(&*keyword)
                }
            },
            Self::Type(syntax) => {
                dest.write_str("type(")?;
                dest.write_str(&*syntax)?;
                dest.write_char(')')
            },
        }
    }
}

/// Lookup scope for an `attr(...)` resolution.
///
/// moegoe Family 14: PDFreactor extends standard `attr()` with
/// `-ro-attr(name, ancestor)` and `-ro-attr-ancestor(name)`. Both
/// resolve to the value of the named attribute on the nearest
/// ancestor that carries it. The compat translator rewrites the
/// PDFreactor spellings to `-bd-attr` / `-bd-attr-ancestor` before
/// Stylo parses; the function-name dispatch below tags the
/// resulting `Attr` with the appropriate scope so downstream
/// (`moegoe-css::computed_to_ir`) can choose `self` vs ancestor
/// lookup.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub enum AttrScope {
    /// Standard CSS `attr()` — looks up on the element itself.
    #[default]
    SelfElement,
    /// `-bd-attr-ancestor()` or `-bd-attr(name, ancestor)` —
    /// looks up on the nearest ancestor with the named attribute.
    Ancestor,
}

/// An attr(...) rule.
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[css(function)]
#[repr(C)]
pub struct Attr {
    /// Namespace-aware attribute name.
    pub name: AttrName,
    /// Parsed attr type / unit syntax.
    pub syntax: AttrSyntax,
    /// Fallback value
    pub fallback: crate::OwnedStr,
    /// Lookup scope — standard `attr()` is `SelfElement`;
    /// `-bd-attr-ancestor()` and `-bd-attr(name, ancestor)` set
    /// `Ancestor`. moegoe Family 14.
    pub scope: AttrScope,
}

impl Parse for Attr {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Attr, ParseError<'i>> {
        // moegoe Family 14: accept `attr`, `-bd-attr`, and
        // `-bd-attr-ancestor` as the dispatch function name. The
        // first two follow the standard grammar (with `ancestor`
        // permitted as an additional positional keyword on
        // `-bd-attr`); the third forces ancestor lookup.
        let function = input.expect_function()?.clone();
        let scope = match_ignore_ascii_case! { &function,
            "attr" => AttrScope::SelfElement,
            "-bd-attr" => AttrScope::SelfElement,
            "-bd-attr-ancestor" => AttrScope::Ancestor,
            _ => return Err(input.new_custom_error(
                StyleParseErrorKind::UnspecifiedError,
            )),
        };
        input.parse_nested_block(|i| Attr::parse_function_with_scope(context, i, scope))
    }
}

impl Attr {
    /// Parse contents of attr() assuming we have already parsed `attr` and are
    /// within a parse_nested_block().
    ///
    /// Standard CSS `attr()` — keeps `AttrScope::SelfElement`.
    pub fn parse_function<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Attr, ParseError<'i>> {
        Self::parse_function_with_scope(context, input, AttrScope::SelfElement)
    }

    /// Parse contents of `attr(...)` / `-bd-attr(...)` /
    /// `-bd-attr-ancestor(...)` assuming we have already consumed
    /// the function name and are within a `parse_nested_block()`.
    ///
    /// Grammar: `[namespace? '|']? ident [, ancestor]? [attr-type]?
    /// [, fallback]?`. The `, ancestor` positional keyword is a
    /// moegoe Family 14 extension on `-bd-attr` (and accepted on
    /// `attr` for ergonomic parity); when present it forces
    /// `AttrScope::Ancestor`. The `-bd-attr-ancestor` function
    /// passes `AttrScope::Ancestor` directly and rejects the
    /// keyword.
    pub fn parse_function_with_scope<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        mut scope: AttrScope,
    ) -> Result<Attr, ParseError<'i>> {
        let name = AttrName::parse_with_namespaces(input, &context.namespaces)?;

        // moegoe Family 14: optional `, ancestor` keyword on
        // `attr()` / `-bd-attr()`. Forces ancestor lookup.
        if input
            .try_parse(|i| -> Result<(), ParseError<'i>> {
                i.expect_comma()?;
                i.expect_ident_matching("ancestor")?;
                Ok(())
            })
            .is_ok()
        {
            scope = AttrScope::Ancestor;
        }

        let syntax = input.try_parse(parse_attr_syntax).unwrap_or_default();
        let fallback = input
            .try_parse(|input| -> Result<crate::OwnedStr, ParseError<'i>> {
                input.expect_comma()?;
                parse_attr_fallback(input)
            })
            .unwrap_or_default();
        input.expect_exhausted()?;

        Ok(Attr {
            name,
            syntax,
            fallback,
            scope,
        })
    }
}

fn is_legacy_attr_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "string"
            | "url"
            | "color"
            | "integer"
            | "length"
            | "angle"
            | "time"
            | "frequency"
            | "-bd-ident"
    )
}

fn parse_attr_syntax<'i, 't>(input: &mut Parser<'i, 't>) -> Result<AttrSyntax, ParseError<'i>> {
    let token = input.next()?.clone();
    match token {
        Token::Function(ref name) if name.eq_ignore_ascii_case("type") => {
            let syntax = input.parse_nested_block(Descriptor::from_css_parser)?;
            Ok(AttrSyntax::Type(syntax.to_css_string().into()))
        },
        Token::Ident(ref ident) => Ok(match_ignore_ascii_case! { ident,
            "raw-string" => AttrSyntax::RawString,
            _ => {
                if AttrUnit::from_ident(ident).is_ok() || is_legacy_attr_keyword(ident.as_ref()) {
                    AttrSyntax::Keyword(ident.as_ref().to_owned().into())
                } else {
                    let ident = ident.clone();
                    return Err(input.new_custom_error(
                        StyleParseErrorKind::UnexpectedIdent(ident)
                    ));
                }
            },
        }),
        Token::Delim('%') => Ok(AttrSyntax::Keyword(String::from("%").into())),
        token => Err(input.new_unexpected_token_error(token)),
    }
}

fn parse_attr_fallback<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<crate::OwnedStr, ParseError<'i>> {
    let start = input.position();
    while input.next_including_whitespace_and_comments().is_ok() {}
    let fallback = input.slice_from(start).trim();
    if fallback.is_empty() {
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(fallback.to_owned().into())
}

impl ToCss for Attr {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        // moegoe Family 14: round-trip `-bd-attr-ancestor()` for
        // ancestor-scoped attr() rules; otherwise serialise the
        // standard `attr(... , ancestor)` shape so the value can
        // be read back by a non-moegoe consumer too.
        match self.scope {
            AttrScope::SelfElement => dest.write_str("attr(")?,
            AttrScope::Ancestor => dest.write_str("-bd-attr-ancestor(")?,
        }
        self.name.to_css(dest)?;

        if self.syntax != AttrSyntax::None {
            dest.write_char(' ')?;
            self.syntax.to_css(dest)?;
        }

        if !self.fallback.is_empty() {
            dest.write_str(", ")?;
            dest.write_str(self.fallback.as_ref())?;
        }

        dest.write_char(')')
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use ::url::Url;
    use cssparser::ParserInput;
    use style_traits::{ParsingMode, ToCss};

    fn parse_attr(css: &str) -> Attr {
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| Attr::parse(&context, input))
            .expect("attr() should parse")
    }

    fn parse_non_negative_integer(css: &str) -> Option<Integer> {
        let url_data = UrlExtraData::from(Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| Integer::parse_non_negative(&context, input))
            .ok()
    }

    #[test]
    fn calculated_integer_clamps_to_its_context_range() {
        assert!(parse_non_negative_integer("-1").is_none());

        let calculated = parse_non_negative_integer("calc(-1)").unwrap();
        assert_eq!(calculated.value(), 0);
        assert_eq!(calculated.to_css_string(), "calc(-1)");
        assert!(Integer::new(1) > calculated);
    }

    #[test]
    fn attr_parses_legacy_type_and_string_fallback() {
        let attr = parse_attr(r#"attr(data-status string, "unknown")"#);
        assert_eq!(attr.name.local_name().as_ref(), "data-status");
        assert_eq!(
            attr.syntax,
            AttrSyntax::Keyword(String::from("string").into())
        );
        assert_eq!(&*attr.fallback, r#""unknown""#);
        assert_eq!(
            attr.to_css_string(),
            r#"attr(data-status string, "unknown")"#
        );
    }

    #[test]
    fn attr_parses_type_function_and_raw_fallback() {
        let attr = parse_attr(r#"attr(data-width type(<length-percentage>), 100%)"#);
        assert_eq!(attr.name.local_name().as_ref(), "data-width");
        assert_eq!(
            attr.syntax,
            AttrSyntax::Type(String::from("<length-percentage>").into())
        );
        assert_eq!(&*attr.fallback, "100%");
        assert_eq!(
            attr.to_css_string(),
            r#"attr(data-width type(<length-percentage>), 100%)"#
        );
    }

    #[test]
    fn attr_bd_ancestor_function_parses_and_round_trips() {
        let attr = parse_attr(r#"-bd-attr-ancestor(data-section)"#);
        assert_eq!(attr.name.local_name().as_ref(), "data-section");
        assert_eq!(attr.scope, AttrScope::Ancestor);
        assert_eq!(attr.to_css_string(), r#"-bd-attr-ancestor(data-section)"#);
    }

    #[test]
    fn attr_bd_attr_with_ancestor_keyword_promotes_scope() {
        let attr = parse_attr(r#"-bd-attr(data-section, ancestor)"#);
        assert_eq!(attr.scope, AttrScope::Ancestor);
        assert_eq!(attr.to_css_string(), r#"-bd-attr-ancestor(data-section)"#);
    }

    #[test]
    fn standard_attr_keeps_self_scope() {
        let attr = parse_attr(r#"attr(title)"#);
        assert_eq!(attr.scope, AttrScope::SelfElement);
        assert_eq!(attr.to_css_string(), r#"attr(title)"#);
    }
}
