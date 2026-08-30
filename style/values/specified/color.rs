/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified color values.

use super::AllowQuirks;
use crate::color::mix::ColorInterpolationMethod;
use crate::color::{parsing, AbsoluteColor, ColorFunction, ColorMixItemList, ColorSpace};
use crate::derives::*;
use crate::media_queries::Device;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{Color as ComputedColor, Context, ToComputedValue};
use crate::values::generics::color::{
    ColorMixFlags, GenericCaretColor, GenericColorMix, GenericColorMixItem,
    GenericColorMixPercentage, GenericColorOrAuto, GenericLightDark,
};
use crate::values::specified::Percentage;
use crate::values::{normalize, CustomIdent};
use cssparser::{match_ignore_ascii_case, BasicParseErrorKind, ParseErrorKind, Parser, Token};
use std::fmt::{self, Write};
use std::io::Write as IoWrite;
use style_traits::{
    owned_slice::OwnedSlice, CssType, CssWriter, KeywordsCollectFn, ParseError, SpecifiedValueInfo,
    StyleParseErrorKind, ToCss, ValueParseErrorKind,
};

/// A specified color-mix().
pub type ColorMix = GenericColorMix<Color, Percentage>;

impl ColorMix {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        preserve_authored: PreserveAuthored,
    ) -> Result<Self, ParseError<'i>> {
        input.expect_function_matching("color-mix")?;

        input.parse_nested_block(|input| {
            // If the color interpolation method is omitted, default to "in oklab".
            // See: https://github.com/web-platform-tests/interop/issues/1166
            let interpolation = input
                .try_parse(|input| -> Result<_, ParseError<'i>> {
                    let interpolation = ColorInterpolationMethod::parse(context, input)?;
                    input.expect_comma()?;
                    Ok(interpolation)
                })
                .unwrap_or_default();

            let try_parse_percentage = |input: &mut Parser| -> Option<Percentage> {
                input
                    .try_parse(|input| Percentage::parse_zero_to_a_hundred(context, input))
                    .ok()
            };

            let mut items = ColorMixItemList::default();

            loop {
                let mut percentage = try_parse_percentage(input);

                let color = Color::parse_internal(context, input, preserve_authored)?;

                if percentage.is_none() {
                    percentage = try_parse_percentage(input);
                }

                if percentage
                    .as_ref()
                    .is_some_and(|value| value.resolve().is_none())
                {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }

                items.push((color, percentage));

                if input.try_parse(|i| i.expect_comma()).is_err() {
                    break;
                }
            }

            // Normalize percentages per:
            // https://drafts.csswg.org/css-values-5/#normalize-mix-percentages
            let (mut sum_specified, mut missing) = (0.0, 0);
            for (_, percentage) in items.iter() {
                if let Some(p) = percentage {
                    sum_specified += p.resolve().unwrap();
                } else {
                    missing += 1;
                }
            }

            let default_for_missing_items = match missing {
                0 => None,
                m if m == items.len() => Some(Percentage::new(1.0 / items.len() as f32)),
                m => Some(Percentage::new((1.0 - sum_specified.min(1.0)) / m as f32)),
            };

            let finalized = items
                .into_iter()
                .map(|(color, percentage)| {
                    let percentage = match percentage {
                        Some(value) => GenericColorMixPercentage::Explicit(value),
                        None => GenericColorMixPercentage::Implied(
                            default_for_missing_items
                                .as_ref()
                                .expect("an omitted percentage has a default")
                                .clone(),
                        ),
                    };
                    GenericColorMixItem { color, percentage }
                })
                .collect::<ColorMixItemList<_>>();

            // Pass RESULT_IN_MODERN_SYNTAX here, because the result of the color-mix() function
            // should always be in the modern color syntax to allow for out of gamut results and
            // to preserve floating point precision.
            ColorMix::new(
                interpolation,
                OwnedSlice::from_slice(&finalized),
                ColorMixFlags::NORMALIZE_WEIGHTS | ColorMixFlags::RESULT_IN_MODERN_SYNTAX,
            )
            .ok_or_else(|| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
        })
    }
}

/// Container holding an absolute color and the text specified by an author.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct Absolute {
    /// The specified color.
    pub color: AbsoluteColor,
    /// Authored representation.
    pub authored: Option<Box<str>>,
}

impl ToCss for Absolute {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if let Some(ref authored) = self.authored {
            dest.write_str(authored)
        } else {
            self.color.to_css(dest)
        }
    }
}

/// Specified color value
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem, ToTyped)]
pub enum Color {
    /// The 'currentColor' keyword
    CurrentColor,
    /// An absolute color.
    /// https://w3c.github.io/csswg-drafts/css-color-4/#typedef-absolute-color-function
    Absolute(Box<Absolute>),
    /// A color function that could not be resolved to a [Color::Absolute] color at parse time.
    /// Right now this is only the case for relative colors with `currentColor` as the origin.
    ColorFunction(Box<ColorFunction<Self>>),
    /// A system color.
    System(SystemColor),
    /// A color mix.
    ColorMix(Box<ColorMix>),
    /// A light-dark() color.
    LightDark(Box<GenericLightDark<Self>>),
    /// The contrast-color function.
    ContrastColor(Box<Color>),
    /// Quirksmode-only rule for inheriting color from the body
    #[cfg(feature = "gecko")]
    InheritFromBodyQuirk,
}

impl From<AbsoluteColor> for Color {
    #[inline]
    fn from(value: AbsoluteColor) -> Self {
        Self::from_absolute_color(value)
    }
}

/// System colors. A bunch of these are ad-hoc, others come from Windows:
///
///   https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsyscolor
///
/// Others are HTML/CSS specific. Spec is:
///
///   https://drafts.csswg.org/css-color/#css-system-colors
///   https://drafts.csswg.org/css-color/#deprecated-system-colors
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, MallocSizeOf, Parse, PartialEq, ToCss, ToShmem)]
#[repr(u8)]
pub enum SystemColor {
    Activeborder,
    /// Background in the (active) titlebar.
    Activecaption,
    Appworkspace,
    Background,
    Buttonface,
    Buttonhighlight,
    Buttonshadow,
    Buttontext,
    Buttonborder,
    /// Text color in the (active) titlebar.
    Captiontext,
    #[parse(aliases = "-moz-field")]
    Field,
    /// Used for disabled field backgrounds.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozDisabledfield,
    #[parse(aliases = "-moz-fieldtext")]
    Fieldtext,

    Mark,
    Marktext,

    /// Combobox widgets
    MozComboboxtext,
    MozCombobox,

    Graytext,
    Highlight,
    Highlighttext,
    Inactiveborder,
    /// Background in the (inactive) titlebar.
    Inactivecaption,
    /// Text color in the (inactive) titlebar.
    Inactivecaptiontext,
    Infobackground,
    Infotext,
    Menu,
    Menutext,
    Scrollbar,
    Threeddarkshadow,
    Threedface,
    Threedhighlight,
    Threedlightshadow,
    Threedshadow,
    Window,
    Windowframe,
    Windowtext,
    #[parse(aliases = "-moz-default-color")]
    Canvastext,
    #[parse(aliases = "-moz-default-background-color")]
    Canvas,
    MozDialog,
    MozDialogtext,
    /// Used for selected but not focused cell backgrounds.
    #[parse(aliases = "-moz-html-cellhighlight")]
    MozCellhighlight,
    /// Used for selected but not focused cell text.
    #[parse(aliases = "-moz-html-cellhighlighttext")]
    MozCellhighlighttext,
    /// Used for selected and focused html cell backgrounds.
    Selecteditem,
    /// Used for selected and focused html cell text.
    Selecteditemtext,
    /// Used for menu item backgrounds when hovered.
    MozMenuhover,
    /// Used for menu item backgrounds when hovered and disabled.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozMenuhoverdisabled,
    /// Used for menu item text when hovered.
    MozMenuhovertext,
    /// Used for menubar item text when hovered.
    MozMenubarhovertext,

    /// On platforms where this color is the same as field, or transparent, use fieldtext as
    /// foreground color.
    MozOddtreerow,

    /// Used for button text background when hovered.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonhoverface,
    /// Used for button text color when hovered.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonhovertext,
    /// Used for button border color when hovered.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonhoverborder,
    /// Used for button background when pressed.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonactiveface,
    /// Used for button text when pressed.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonactivetext,
    /// Used for button border when pressed.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtonactiveborder,

    /// Used for button background when disabled.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtondisabledface,
    /// Used for button border when disabled.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozButtondisabledborder,

    /// Colors used for the header bar (sorta like the tab bar / menubar).
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozHeaderbar,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozHeaderbartext,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozHeaderbarinactive,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozHeaderbarinactivetext,

    /// Foreground color of default buttons.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozMacDefaultbuttontext,
    /// Ring color around text fields and lists.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozMacFocusring,
    /// Text color of disabled text on toolbars.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozMacDisabledtoolbartext,
    /// The background of a sidebar.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozSidebar,
    /// The foreground color of a sidebar.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozSidebartext,
    /// The border color of a sidebar.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozSidebarborder,

    /// Theme accent color.
    /// https://drafts.csswg.org/css-color-4/#valdef-system-color-accentcolor
    Accentcolor,

    /// Foreground for the accent color.
    /// https://drafts.csswg.org/css-color-4/#valdef-system-color-accentcolortext
    Accentcolortext,

    /// The background-color for :autofill-ed inputs.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozAutofillBackground,

    #[parse(aliases = "-moz-hyperlinktext")]
    Linktext,
    #[parse(aliases = "-moz-activehyperlinktext")]
    Activetext,
    #[parse(aliases = "-moz-visitedhyperlinktext")]
    Visitedtext,

    /// Color of tree column headers
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheader,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheadertext,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheaderhover,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheaderhovertext,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheaderactive,
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    MozColheaderactivetext,

    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    TextSelectDisabledBackground,
    #[css(skip)]
    TextSelectAttentionBackground,
    #[css(skip)]
    TextSelectAttentionForeground,
    #[css(skip)]
    TextHighlightBackground,
    #[css(skip)]
    TextHighlightForeground,
    #[css(skip)]
    TargetTextBackground,
    #[css(skip)]
    TargetTextForeground,
    #[css(skip)]
    IMERawInputBackground,
    #[css(skip)]
    IMERawInputForeground,
    #[css(skip)]
    IMERawInputUnderline,
    #[css(skip)]
    IMESelectedRawTextBackground,
    #[css(skip)]
    IMESelectedRawTextForeground,
    #[css(skip)]
    IMESelectedRawTextUnderline,
    #[css(skip)]
    IMEConvertedTextBackground,
    #[css(skip)]
    IMEConvertedTextForeground,
    #[css(skip)]
    IMEConvertedTextUnderline,
    #[css(skip)]
    IMESelectedConvertedTextBackground,
    #[css(skip)]
    IMESelectedConvertedTextForeground,
    #[css(skip)]
    IMESelectedConvertedTextUnderline,
    #[css(skip)]
    SpellCheckerUnderline,
    #[css(skip)]
    ThemedScrollbar,
    #[css(skip)]
    ThemedScrollbarThumb,
    #[css(skip)]
    ThemedScrollbarThumbHover,
    #[css(skip)]
    ThemedScrollbarThumbActive,

    #[css(skip)]
    End, // Just for array-indexing purposes.
}

impl SystemColor {
    fn modern_equivalent(self) -> Self {
        match self {
            Self::Activeborder
            | Self::Inactiveborder
            | Self::Threeddarkshadow
            | Self::Threedhighlight
            | Self::Threedlightshadow
            | Self::Threedshadow
            | Self::Windowframe => Self::Buttonborder,
            Self::Activecaption
            | Self::Appworkspace
            | Self::Background
            | Self::Inactivecaption
            | Self::Infobackground
            | Self::Menu
            | Self::Scrollbar
            | Self::Window => Self::Canvas,
            Self::Buttonhighlight | Self::Buttonshadow | Self::Threedface => Self::Buttonface,
            Self::Captiontext | Self::Infotext | Self::Menutext | Self::Windowtext => {
                Self::Canvastext
            },
            Self::Inactivecaptiontext => Self::Graytext,
            modern => modern,
        }
    }

    #[cfg(feature = "servo")]
    #[inline]
    fn compute(&self, cx: &Context) -> ComputedColor {
        let background = cx.device().default_background_color();
        let foreground = cx.device().default_color();
        let accent = AbsoluteColor::srgb_legacy(0, 120, 212, 1.0);
        let accent_text = AbsoluteColor::WHITE;
        let highlight = AbsoluteColor::srgb_legacy(51, 144, 255, 1.0);
        let highlight_text = AbsoluteColor::WHITE;
        let button_face = AbsoluteColor::srgb_legacy(240, 240, 240, 1.0);
        let button_border = AbsoluteColor::srgb_legacy(118, 118, 118, 1.0);
        let light_shadow = AbsoluteColor::srgb_legacy(211, 211, 211, 1.0);
        let dark_shadow = AbsoluteColor::srgb_legacy(64, 64, 64, 1.0);
        let disabled_text = AbsoluteColor::srgb_legacy(128, 128, 128, 1.0);
        let info_background = AbsoluteColor::srgb_legacy(255, 255, 225, 1.0);
        let mark = AbsoluteColor::srgb_legacy(255, 255, 0, 1.0);

        let absolute = match self.modern_equivalent() {
            Self::Canvas
            | Self::Window
            | Self::Field
            | Self::Menu
            | Self::MozDialog
            | Self::MozCombobox
            | Self::Background => background,
            Self::Canvastext
            | Self::Windowtext
            | Self::Buttontext
            | Self::Fieldtext
            | Self::Captiontext
            | Self::Inactivecaptiontext
            | Self::Infotext
            | Self::Menutext
            | Self::MozDialogtext
            | Self::MozComboboxtext
            | Self::MozOddtreerow
            | Self::Marktext => foreground,
            // Keep the fixed print palette consistent with the Servo UA link colour.
            Self::Linktext => AbsoluteColor::srgb_legacy(0, 0, 255, 1.0),
            Self::Visitedtext => AbsoluteColor::srgb_legacy(85, 26, 139, 1.0),
            Self::Activetext => AbsoluteColor::srgb_legacy(255, 0, 0, 1.0),
            Self::Accentcolor => accent,
            Self::Accentcolortext => accent_text,
            Self::Highlight
            | Self::Selecteditem
            | Self::MozMenuhover
            | Self::MozCellhighlight
            | Self::TextHighlightBackground
            | Self::TargetTextBackground
            | Self::TextSelectAttentionBackground => highlight,
            Self::Highlighttext
            | Self::Selecteditemtext
            | Self::MozMenuhovertext
            | Self::MozMenubarhovertext
            | Self::MozCellhighlighttext
            | Self::TextHighlightForeground
            | Self::TargetTextForeground
            | Self::TextSelectAttentionForeground => highlight_text,
            Self::Buttonface
            | Self::Threedface
            | Self::MozButtondisabledface
            | Self::MozHeaderbar
            | Self::MozHeaderbarinactive
            | Self::Appworkspace
            | Self::Scrollbar => button_face,
            Self::Buttonborder
            | Self::Activeborder
            | Self::Inactiveborder
            | Self::MozButtonhoverborder
            | Self::MozButtonactiveborder
            | Self::MozButtondisabledborder
            | Self::MozSidebarborder => button_border,
            Self::Graytext
            | Self::Buttonshadow
            | Self::Threedshadow
            | Self::TextSelectDisabledBackground => disabled_text,
            Self::Buttonhighlight | Self::Threedhighlight => AbsoluteColor::WHITE,
            Self::Threedlightshadow => light_shadow,
            Self::Threeddarkshadow | Self::Windowframe => dark_shadow,
            Self::Mark => mark,
            Self::Infobackground => info_background,
            Self::Activecaption
            | Self::Inactivecaption
            | Self::MozButtonhoverface
            | Self::MozButtonactiveface
            | Self::MozHeaderbartext
            | Self::MozHeaderbarinactivetext
            | Self::MozMacFocusring
            | Self::MozSidebar
            | Self::MozAutofillBackground
            | Self::MozColheader
            | Self::MozColheaderhover
            | Self::MozColheaderactive => accent,
            Self::MozButtonhovertext
            | Self::MozButtonactivetext
            | Self::MozMacDefaultbuttontext
            | Self::MozMacDisabledtoolbartext
            | Self::MozSidebartext
            | Self::MozColheadertext
            | Self::MozColheaderhovertext
            | Self::MozColheaderactivetext => accent_text,
            Self::MozDisabledfield => button_face,
            _ => foreground,
        };

        ComputedColor::Absolute(absolute)
    }

    #[cfg(feature = "gecko")]
    #[inline]
    fn compute(&self, cx: &Context) -> ComputedColor {
        use crate::gecko::values::convert_nscolor_to_absolute_color;
        use crate::gecko_bindings::bindings;

        let color = cx
            .device()
            .system_nscolor(self.modern_equivalent(), cx.builder.color_scheme);
        if cx.for_non_inherited_property {
            cx.rule_cache_conditions
                .borrow_mut()
                .set_color_scheme_dependency(cx.builder.color_scheme);
        }
        if color == bindings::NS_SAME_AS_FOREGROUND_COLOR {
            return ComputedColor::currentcolor();
        }
        ComputedColor::Absolute(convert_nscolor_to_absolute_color(color))
    }
}

#[cfg(test)]
mod system_color_tests {
    use super::SystemColor;

    #[test]
    fn deprecated_system_colours_compute_as_their_modern_equivalents() {
        for (deprecated, modern) in [
            (SystemColor::Activeborder, SystemColor::Buttonborder),
            (SystemColor::Activecaption, SystemColor::Canvas),
            (SystemColor::Appworkspace, SystemColor::Canvas),
            (SystemColor::Background, SystemColor::Canvas),
            (SystemColor::Buttonhighlight, SystemColor::Buttonface),
            (SystemColor::Buttonshadow, SystemColor::Buttonface),
            (SystemColor::Captiontext, SystemColor::Canvastext),
            (SystemColor::Inactiveborder, SystemColor::Buttonborder),
            (SystemColor::Inactivecaption, SystemColor::Canvas),
            (SystemColor::Inactivecaptiontext, SystemColor::Graytext),
            (SystemColor::Infobackground, SystemColor::Canvas),
            (SystemColor::Infotext, SystemColor::Canvastext),
            (SystemColor::Menu, SystemColor::Canvas),
            (SystemColor::Menutext, SystemColor::Canvastext),
            (SystemColor::Scrollbar, SystemColor::Canvas),
            (SystemColor::Threeddarkshadow, SystemColor::Buttonborder),
            (SystemColor::Threedface, SystemColor::Buttonface),
            (SystemColor::Threedhighlight, SystemColor::Buttonborder),
            (SystemColor::Threedlightshadow, SystemColor::Buttonborder),
            (SystemColor::Threedshadow, SystemColor::Buttonborder),
            (SystemColor::Window, SystemColor::Canvas),
            (SystemColor::Windowframe, SystemColor::Buttonborder),
            (SystemColor::Windowtext, SystemColor::Canvastext),
        ] {
            assert_eq!(deprecated.modern_equivalent(), modern);
        }
    }
}

/// Whether to preserve authored colors during parsing. That's useful only if we
/// plan to serialize the color back.
#[derive(Copy, Clone)]
enum PreserveAuthored {
    No,
    Yes,
}

impl Parse for Color {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_internal(context, input, PreserveAuthored::Yes)
    }
}

impl Color {
    fn parse_internal<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        preserve_authored: PreserveAuthored,
    ) -> Result<Self, ParseError<'i>> {
        let authored = match preserve_authored {
            PreserveAuthored::No => None,
            PreserveAuthored::Yes => {
                // Currently we only store authored value for color keywords,
                // because all browsers serialize those values as keywords for
                // specified value.
                let start = input.state();
                let authored = input.expect_ident_cloned().ok();
                input.reset(&start);
                authored
            },
        };

        match input.try_parse(|i| parsing::parse_color_with(context, i)) {
            Ok(mut color) => {
                if let Color::Absolute(ref mut absolute) = color {
                    // Because we can't set the `authored` value at construction time, we have to set it
                    // here.
                    absolute.authored = authored.map(|s| s.to_ascii_lowercase().into_boxed_str());
                }
                Ok(color)
            },
            Err(e) => {
                if let Ok(system) = input.try_parse(|i| SystemColor::parse(context, i)) {
                    return Ok(Color::System(system));
                }

                if let Ok(mix) = input.try_parse(|i| ColorMix::parse(context, i, preserve_authored))
                {
                    return Ok(Color::ColorMix(Box::new(mix)));
                }

                if let Ok(ld) = input.try_parse(|i| {
                    GenericLightDark::parse_with(i, |i| {
                        Self::parse_internal(context, i, preserve_authored)
                    })
                }) {
                    return Ok(Color::LightDark(Box::new(ld)));
                }

                if static_prefs::pref!("layout.css.contrast-color.enabled") {
                    if let Ok(c) = input.try_parse(|i| {
                        i.expect_function_matching("contrast-color")?;
                        i.parse_nested_block(|i| {
                            Self::parse_internal(context, i, preserve_authored)
                        })
                    }) {
                        return Ok(Color::ContrastColor(Box::new(c)));
                    }
                }

                match e.kind {
                    ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(t)) => {
                        Err(e.location.new_custom_error(StyleParseErrorKind::ValueError(
                            ValueParseErrorKind::InvalidColor(t),
                        )))
                    },
                    _ => Err(e),
                }
            },
        }
    }

    /// Returns whether a given color is valid for authors.
    pub fn is_valid(context: &ParserContext, input: &mut Parser) -> bool {
        input
            .parse_entirely(|input| Self::parse_internal(context, input, PreserveAuthored::No))
            .is_ok()
    }

    /// Tries to parse a color and compute it with a given device.
    pub fn parse_and_compute(
        context: &ParserContext,
        input: &mut Parser,
        device: Option<&Device>,
    ) -> Option<ComputedColor> {
        use crate::error_reporting::ContextualParseError;
        let start = input.position();
        let result = input
            .parse_entirely(|input| Self::parse_internal(context, input, PreserveAuthored::No));

        let specified = match result {
            Ok(s) => s,
            Err(e) => {
                if !context.error_reporting_enabled() {
                    return None;
                }
                // Ignore other kinds of errors that might be reported, such as
                // ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken),
                // since Gecko didn't use to report those to the error console.
                //
                // TODO(emilio): Revise whether we want to keep this at all, we
                // use this only for canvas, this warnings are disabled by
                // default and not available on OffscreenCanvas anyways...
                if let ParseErrorKind::Custom(StyleParseErrorKind::ValueError(..)) = e.kind {
                    let location = e.location.clone();
                    let error = ContextualParseError::UnsupportedValue(input.slice_from(start), e);
                    context.log_css_error(location, error);
                }
                return None;
            },
        };

        match device {
            Some(device) => {
                Context::for_media_query_evaluation(device, device.quirks_mode(), |context| {
                    specified.to_computed_color(Some(&context))
                })
            },
            None => specified.to_computed_color(None),
        }
    }
}

impl ToCss for Color {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match *self {
            Color::CurrentColor => dest.write_str("currentcolor"),
            Color::Absolute(ref absolute) => absolute.to_css(dest),
            Color::ColorFunction(ref color_function) => color_function.to_css(dest),
            Color::ColorMix(ref mix) => mix.to_css(dest),
            Color::LightDark(ref ld) => ld.to_css(dest),
            Color::ContrastColor(ref c) => {
                dest.write_str("contrast-color(")?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
            Color::System(system) => system.to_css(dest),
            #[cfg(feature = "gecko")]
            Color::InheritFromBodyQuirk => dest.write_str("-moz-inherit-from-body-quirk"),
        }
    }
}

impl Color {
    /// Returns whether this color is allowed in forced-colors mode.
    pub fn honored_in_forced_colors_mode(&self, allow_transparent: bool) -> bool {
        match *self {
            #[cfg(feature = "gecko")]
            Self::InheritFromBodyQuirk => false,
            Self::CurrentColor => true,
            Self::System(..) => true,
            Self::Absolute(ref absolute) => allow_transparent && absolute.color.is_transparent(),
            Self::ColorFunction(ref color_function) => {
                // For now we allow transparent colors if we can resolve the color function.
                // <https://bugzilla.mozilla.org/show_bug.cgi?id=1923053>
                color_function
                    .resolve_to_absolute()
                    .map(|resolved| allow_transparent && resolved.is_transparent())
                    .unwrap_or(false)
            },
            Self::LightDark(ref ld) => {
                ld.light.honored_in_forced_colors_mode(allow_transparent)
                    && ld.dark.honored_in_forced_colors_mode(allow_transparent)
            },
            Self::ColorMix(ref mix) => mix
                .items()
                .iter()
                .all(|item| item.color.honored_in_forced_colors_mode(allow_transparent)),
            Self::ContrastColor(ref c) => c.honored_in_forced_colors_mode(allow_transparent),
        }
    }

    /// Returns currentcolor value.
    #[inline]
    pub fn currentcolor() -> Self {
        Self::CurrentColor
    }

    /// Returns transparent value.
    #[inline]
    pub fn transparent() -> Self {
        // We should probably set authored to "transparent", but maybe it doesn't matter.
        Self::from_absolute_color(AbsoluteColor::TRANSPARENT_BLACK)
    }

    /// Create a color from an [`AbsoluteColor`].
    pub fn from_absolute_color(color: AbsoluteColor) -> Self {
        Color::Absolute(Box::new(Absolute {
            color,
            authored: None,
        }))
    }

    /// Resolve this Color into an AbsoluteColor if it does not use any of the
    /// forms that are invalid in an absolute color.
    ///   https://drafts.csswg.org/css-color-5/#absolute-color
    /// Returns None if the specified color is not valid as an absolute color.
    pub fn resolve_to_absolute(&self) -> Option<AbsoluteColor> {
        match self {
            Self::Absolute(c) => Some(c.color),
            Self::ColorFunction(ref color_function) => color_function.resolve_to_absolute().ok(),
            Self::ColorMix(ref mix) => {
                use crate::color::mix;

                let mut items = ColorMixItemList::with_capacity(mix.items().len());
                for item in mix.items() {
                    items.push(mix::ColorMixItem::new(
                        item.color.resolve_to_absolute()?,
                        item.percentage.value().resolve()?,
                    ))
                }

                Some(mix::mix_many(mix.interpolation, items, mix.flags))
            },
            _ => None,
        }
    }

    /// Parse a color, with quirks.
    ///
    /// <https://quirks.spec.whatwg.org/#the-hashless-hex-color-quirk>
    pub fn parse_quirky<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allow_quirks: AllowQuirks,
    ) -> Result<Self, ParseError<'i>> {
        input.try_parse(|i| Self::parse(context, i)).or_else(|e| {
            if !allow_quirks.allowed(context.quirks_mode) {
                return Err(e);
            }
            Color::parse_quirky_color(input).map_err(|_| e)
        })
    }

    fn parse_hash<'i>(
        bytes: &[u8],
        loc: &cssparser::SourceLocation,
    ) -> Result<Self, ParseError<'i>> {
        match cssparser::color::parse_hash_color(bytes) {
            Ok((r, g, b, a)) => Ok(Self::from_absolute_color(AbsoluteColor::srgb_legacy(
                r, g, b, a,
            ))),
            Err(()) => Err(loc.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }

    /// Parse a <quirky-color> value.
    ///
    /// <https://quirks.spec.whatwg.org/#the-hashless-hex-color-quirk>
    fn parse_quirky_color<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let (value, unit) = match *input.next()? {
            Token::Number {
                int_value: Some(integer),
                ..
            } => (integer, None),
            Token::Dimension {
                int_value: Some(integer),
                ref unit,
                ..
            } => (integer, Some(unit)),
            Token::Ident(ref ident) => {
                if ident.len() != 3 && ident.len() != 6 {
                    return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                return Self::parse_hash(ident.as_bytes(), &location);
            },
            ref t => {
                return Err(location.new_unexpected_token_error(t.clone()));
            },
        };
        if value < 0 {
            return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let length = if value <= 9 {
            1
        } else if value <= 99 {
            2
        } else if value <= 999 {
            3
        } else if value <= 9999 {
            4
        } else if value <= 99999 {
            5
        } else if value <= 999999 {
            6
        } else {
            return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        };
        let total = length + unit.as_ref().map_or(0, |d| d.len());
        if total > 6 {
            return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let mut serialization = [b'0'; 6];
        let space_padding = 6 - total;
        let mut written = space_padding;
        let mut buf = itoa::Buffer::new();
        let s = buf.format(value);
        (&mut serialization[written..])
            .write_all(s.as_bytes())
            .unwrap();
        written += s.len();
        if let Some(unit) = unit {
            written += (&mut serialization[written..])
                .write(unit.as_bytes())
                .unwrap();
        }
        debug_assert_eq!(written, 6);
        Self::parse_hash(&serialization, &location)
    }
}

impl Color {
    /// Converts this Color into a ComputedColor.
    ///
    /// If `context` is `None`, and the specified color requires data from
    /// the context to resolve, then `None` is returned.
    pub fn to_computed_color(&self, context: Option<&Context>) -> Option<ComputedColor> {
        macro_rules! adjust_absolute_color {
            ($color:expr) => {{
                // Computed lightness values can not be NaN.
                if matches!(
                    $color.color_space,
                    ColorSpace::Lab | ColorSpace::Oklab | ColorSpace::Lch | ColorSpace::Oklch
                ) {
                    $color.components.0 = normalize($color.components.0);
                }

                // Computed RGB and XYZ components can not be NaN.
                if !$color.is_legacy_syntax() && $color.color_space.is_rgb_or_xyz_like() {
                    $color.components = $color.components.map(normalize);
                }

                $color.alpha = normalize($color.alpha);
            }};
        }

        Some(match *self {
            Color::CurrentColor => ComputedColor::CurrentColor,
            Color::Absolute(ref absolute) => {
                let mut color = absolute.color;
                adjust_absolute_color!(color);
                ComputedColor::Absolute(color)
            },
            Color::ColorFunction(ref color_function) => {
                let color_function = match context {
                    Some(context) => color_function.to_computed_value(context),
                    None => (**color_function).clone(),
                };

                // F2 — keep `device-cmyk(c m y k / a)` with no fallback as a
                // typed `ColorFunction::DeviceCmyk` at compute time so
                // downstream print backends can preserve the CMYK quad
                // through to PDF emission. Without this, the cascade collapses
                // CMYK to its naive sRGB projection per CSS Color 4 §10.2.2
                // and the original ink-coverage data is unrecoverable.
                //
                // Authored fallbacks (e.g. `device-cmyk(0 1 1 0, red)`) keep
                // the eager resolution path because the author explicitly
                // asked for the fallback's colour space whenever CMYK is
                // unavailable.
                if let ColorFunction::DeviceCmyk(_, _, _, _, _, fallback) = &color_function {
                    if fallback.is_none() {
                        let color_function = color_function.map_origin_color(|origin_color| {
                            origin_color.to_computed_color(context)
                        })?;
                        return Some(ComputedColor::ColorFunction(Box::new(color_function)));
                    }
                }

                // F2 — moegoe `-bd-spot(<name>)` / `-bd-separation(<name>)`
                // must survive to compute time so the IR conversion
                // boundary can resolve the colorant against the
                // document `@-bd-colour` registry. Eager resolution at
                // this layer would collapse the named reference to
                // `TRANSPARENT_BLACK` (see `resolve_to_absolute` for
                // the fail-closed contract) and the colorant name
                // would be unrecoverable.
                if matches!(&color_function, ColorFunction::BdSpot(..)) {
                    let color_function = color_function
                        .map_origin_color(|origin_color| origin_color.to_computed_color(context))?;
                    return Some(ComputedColor::ColorFunction(Box::new(color_function)));
                }

                // F2 — moegoe `device-n(<name> <tint>, … , <fallback>)`
                // must also survive to compute time so the IR
                // conversion boundary can preserve the per-colorant
                // tints and resolve them against the document's
                // registered DeviceN-aware backends (Stage C). Eager
                // resolution here would collapse to the fallback sRGB
                // colour and the colorant references would be
                // unrecoverable.
                if matches!(&color_function, ColorFunction::BdDeviceN(..)) {
                    let color_function = color_function
                        .map_origin_color(|origin_color| origin_color.to_computed_color(context))?;
                    return Some(ComputedColor::ColorFunction(Box::new(color_function)));
                }

                // Try to eagerly resolve the color function before making it a computed color.
                if let Ok(absolute) = color_function.resolve_to_absolute() {
                    ComputedColor::Absolute(absolute)
                } else {
                    let color_function = color_function
                        .map_origin_color(|origin_color| origin_color.to_computed_color(context))?;
                    ComputedColor::ColorFunction(Box::new(color_function))
                }
            },
            Color::LightDark(ref ld) => ld.compute(context?),
            Color::ColorMix(ref mix) => {
                use crate::values::computed::percentage::Percentage;

                let mut items = ColorMixItemList::with_capacity(mix.items().len());
                for item in mix.items() {
                    let compute_percentage = |percentage: &crate::values::specified::Percentage| {
                        Some(match context {
                            Some(context) => percentage.to_computed_value(context),
                            None => Percentage(percentage.resolve()?),
                        })
                    };
                    let percentage = match &item.percentage {
                        GenericColorMixPercentage::Explicit(value) => {
                            GenericColorMixPercentage::Explicit(compute_percentage(value)?)
                        },
                        GenericColorMixPercentage::Implied(value) => {
                            GenericColorMixPercentage::Implied(compute_percentage(value)?)
                        },
                    };
                    items.push(GenericColorMixItem {
                        color: item.color.to_computed_color(context)?,
                        percentage,
                    });
                }

                ComputedColor::from_color_mix(
                    GenericColorMix::new(
                        mix.interpolation,
                        OwnedSlice::from_slice(items.as_slice()),
                        mix.flags,
                    )
                    .expect("a computed mix retains its source item"),
                )
            },
            Color::ContrastColor(ref c) => {
                ComputedColor::ContrastColor(Box::new(c.to_computed_color(context)?))
            },
            Color::System(system) => system.compute(context?),
            #[cfg(feature = "gecko")]
            Color::InheritFromBodyQuirk => {
                ComputedColor::Absolute(context?.device().body_text_color())
            },
        })
    }
}

impl ToComputedValue for Color {
    type ComputedValue = ComputedColor;

    fn to_computed_value(&self, context: &Context) -> ComputedColor {
        self.to_computed_color(Some(context)).unwrap_or_else(|| {
            debug_assert!(
                false,
                "Specified color could not be resolved to a computed color!"
            );
            ComputedColor::Absolute(AbsoluteColor::BLACK)
        })
    }

    fn from_computed_value(computed: &ComputedColor) -> Self {
        match *computed {
            ComputedColor::Absolute(ref color) => Self::from_absolute_color(color.clone()),
            ComputedColor::ColorFunction(ref color_function) => {
                let color_function = color_function
                    .map_origin_color(|o| Some(Self::from_computed_value(o)))
                    .expect("computed colour conversion is infallible");
                Self::ColorFunction(Box::new(color_function))
            },
            ComputedColor::CurrentColor => Color::CurrentColor,
            ComputedColor::ColorMix(ref mix) => {
                Color::ColorMix(Box::new(ToComputedValue::from_computed_value(&**mix)))
            },
            ComputedColor::ContrastColor(ref c) => {
                Self::ContrastColor(Box::new(ToComputedValue::from_computed_value(&**c)))
            },
        }
    }
}

impl SpecifiedValueInfo for Color {
    const SUPPORTED_TYPES: u8 = CssType::COLOR;

    fn collect_completion_keywords(f: KeywordsCollectFn) {
        // We are not going to insert all the color names here. Caller and
        // devtools should take care of them. XXX Actually, transparent
        // should probably be handled that way as well.
        // XXX `currentColor` should really be `currentcolor`. But let's
        // keep it consistent with the old system for now.
        f(&[
            "currentColor",
            "transparent",
            "rgb",
            "rgba",
            "hsl",
            "hsla",
            "hwb",
            "color",
            "lab",
            "lch",
            "oklab",
            "oklch",
            "color-mix",
            "contrast-color",
            "light-dark",
        ]);
    }
}

/// Specified value for the "color" property, which resolves the `currentcolor`
/// keyword to the parent color instead of self's color.
#[cfg_attr(feature = "gecko", derive(MallocSizeOf))]
#[derive(Clone, Debug, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
pub struct ColorPropertyValue(pub Color);

impl ToComputedValue for ColorPropertyValue {
    type ComputedValue = AbsoluteColor;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let current_color = context.builder.get_parent_inherited_text().clone_color();
        self.0
            .to_computed_value(context)
            .resolve_to_absolute(&current_color)
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        ColorPropertyValue(Color::from_absolute_color(*computed).into())
    }
}

impl Parse for ColorPropertyValue {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Color::parse_quirky(context, input, AllowQuirks::Yes).map(ColorPropertyValue)
    }
}

/// auto | <color>
pub type ColorOrAuto = GenericColorOrAuto<Color>;

/// caret-color
pub type CaretColor = GenericCaretColor<Color>;

impl Parse for CaretColor {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        ColorOrAuto::parse(context, input).map(GenericCaretColor)
    }
}

/// Various flags to represent the color-scheme property in an efficient
/// way.
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
#[repr(C)]
#[value_info(other_values = "light,dark,only")]
pub struct ColorSchemeFlags(u8);
bitflags! {
    impl ColorSchemeFlags: u8 {
        /// Whether the author specified `light`.
        const LIGHT = 1 << 0;
        /// Whether the author specified `dark`.
        const DARK = 1 << 1;
        /// Whether the author specified `only`.
        const ONLY = 1 << 2;
    }
}

/// <https://drafts.csswg.org/css-color-adjust/#color-scheme-prop>
#[derive(
    Clone,
    Debug,
    Default,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
#[value_info(other_values = "normal")]
pub struct ColorScheme {
    #[ignore_malloc_size_of = "Arc"]
    idents: crate::ArcSlice<CustomIdent>,
    /// The computed bits for the known color schemes (plus the only keyword).
    pub bits: ColorSchemeFlags,
}

impl ColorScheme {
    /// Returns the `normal` value.
    pub fn normal() -> Self {
        Self {
            idents: Default::default(),
            bits: ColorSchemeFlags::empty(),
        }
    }

    /// Returns the raw bitfield.
    pub fn raw_bits(&self) -> u8 {
        self.bits.bits()
    }
}

impl Parse for ColorScheme {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let mut idents = vec![];
        let mut bits = ColorSchemeFlags::empty();

        let mut location = input.current_source_location();
        while let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            let mut is_only = false;
            match_ignore_ascii_case! { &ident,
                "normal" => {
                    if idents.is_empty() && bits.is_empty() {
                        return Ok(Self::normal());
                    }
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                },
                "light" => bits.insert(ColorSchemeFlags::LIGHT),
                "dark" => bits.insert(ColorSchemeFlags::DARK),
                "only" => {
                    if bits.intersects(ColorSchemeFlags::ONLY) {
                        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                    }
                    bits.insert(ColorSchemeFlags::ONLY);
                    is_only = true;
                },
                _ => {},
            };

            if is_only {
                if !idents.is_empty() {
                    // Only is allowed either at the beginning or at the end,
                    // but not in the middle.
                    break;
                }
            } else {
                idents.push(CustomIdent::from_ident(location, &ident, &[])?);
            }
            location = input.current_source_location();
        }

        if idents.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }

        Ok(Self {
            idents: crate::ArcSlice::from_iter(idents.into_iter()),
            bits,
        })
    }
}

impl ToCss for ColorScheme {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.idents.is_empty() {
            debug_assert!(self.bits.is_empty());
            return dest.write_str("normal");
        }
        let mut first = true;
        for ident in self.idents.iter() {
            if !first {
                dest.write_char(' ')?;
            }
            first = false;
            ident.to_css(dest)?;
        }
        if self.bits.intersects(ColorSchemeFlags::ONLY) {
            dest.write_str(" only")?;
        }
        Ok(())
    }
}

/// https://drafts.csswg.org/css-color-adjust/#print-color-adjust
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum PrintColorAdjust {
    /// Ignore backgrounds and darken text.
    Economy,
    /// Respect specified colors.
    Exact,
}

/// https://drafts.csswg.org/css-color-adjust-1/#forced-color-adjust-prop
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum ForcedColorAdjust {
    /// Adjust colors if needed.
    Auto,
    /// Respect specified colors.
    None,
}

/// Possible values for the forced-colors media query.
/// <https://drafts.csswg.org/mediaqueries-5/#forced-colors>
#[derive(Clone, Copy, Debug, FromPrimitive, Parse, PartialEq, ToCss)]
#[repr(u8)]
pub enum ForcedColors {
    /// Page colors are not being forced.
    None,
    /// Page colors would be forced in content.
    #[parse(condition = "ParserContext::chrome_rules_enabled")]
    Requested,
    /// Page colors are being forced.
    Active,
}

impl ForcedColors {
    /// Returns whether forced-colors is active for this page.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}
