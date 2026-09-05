//! Typed CSSOM entry points backed by Stylo's CSS grammars.

pub use stylo_cssom_model as model;
pub mod counter_style_cssom;
pub mod float_diagnostics;
pub mod media_list;
pub mod numeric;
pub mod registration;
pub mod scroll_markers;
pub mod selector_query;
pub mod source;
#[cfg(test)]
mod style_text_boundary_tests;
pub mod stylesheet_analysis;
pub mod stylesheet_input;
pub mod svg_presentation;
pub mod values;
pub mod view_transition;
pub mod view_transition_name_rewrite;

pub mod active_view_transition_selector;
pub mod author_rule_projection;
pub mod authored_rules;
mod base_url;
pub mod compat;
pub mod context;
pub mod css_color_calibrated_rewrite;
pub mod css_color_hdr_fn_rewrite;
pub mod dynamic_range_limit_mix_rewrite;
pub mod hdr_color_rewrite;
pub mod highlight_projection;
pub mod math_display_rewrite;
pub mod named_colours_cmyk;
mod preferences;
pub mod rule_declaration_lowering;
pub mod rule_parser;
pub mod view_transition_root_rewrite;
pub use authored_rules::ValidatedCssRule;
pub use authored_rules::{
    ParsedStylesheet, ValidatedSelectorText, parse_nested_declarations_input,
};
pub use base_url::CssStylesheetBaseUrl;
pub use rule_parser::RuleInput;
pub mod css_identifier;
pub mod css_scan;
pub mod declaration_parser;
pub mod declaration_serialization;
pub mod overlay_transition_rewrite;
pub mod property_schema;
pub mod typed_om;

pub fn is_valid_custom_property_name(name: &str) -> bool {
    let Some(body) = name.strip_prefix("--") else {
        return false;
    };
    !body.is_empty()
        && body.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c == '-'
                || c == '_'
                || (!c.is_ascii() && !c.is_control() && !c.is_whitespace())
        })
}
pub mod position_try_shorthand_rewrite;
pub mod specified;
mod style_fragment_parser;
pub mod symbols_function_rewrite;
pub mod value_serialization;
pub mod webkit_box_orient_rewrite;

pub use symbols_function_rewrite::value_contains_valid_symbols_function;
