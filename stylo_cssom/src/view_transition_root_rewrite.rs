use std::{borrow::Cow, fmt::Write as _};

use cssparser::{Parser, ParserInput, Token};
use style::values::CustomIdent;

use crate::css_scan::{
    advance_past_string_or_comment, find_matching_delimiter, hex_encode, is_ident_continue,
    rewrite_style_rules, rewrite_style_rules_with_opaque_at_rules, split_top_level,
};

pub const INTERNAL_BACKGROUND_PROPERTY: &str = "--moegoe-view-transition-root-background";
pub const INTERNAL_ROOT_GROUP_OPACITY_PROPERTY: &str =
    "--moegoe-view-transition-root-group-opacity";
pub const INTERNAL_IMAGE_PAIR_OPACITY_PROPERTY: &str =
    "--moegoe-view-transition-image-pair-opacity";
pub const INTERNAL_GROUP_PLACEMENT_PREFIX: &str = "--moegoe-view-transition-group-";
pub const INTERNAL_GROUP_TRANSFORM_PROPERTY: &str = "--moegoe-view-transition-group-transform";
pub const INTERNAL_GROUP_GEOMETRY_TRANSFORM_PROPERTY: &str =
    "--moegoe-view-transition-group-geometry-transform";
pub const INTERNAL_GROUP_PAINT_TRANSLATION_PROPERTY: &str =
    "--moegoe-view-transition-group-paint-translation";
pub const INTERNAL_CAPTURE_PAINT_TRANSFORM_PROPERTY: &str =
    "--moegoe-view-transition-capture-paint-transform";
pub const INTERNAL_GROUP_AUTHORED_TRANSFORM_MARKER: &str =
    "--moegoe-view-transition-group-authored-transform";
pub const INTERNAL_GROUP_CHILDREN_BORDER_COLOR_MARKER: &str =
    "--moegoe-view-transition-group-children-border-color";
pub const INTERNAL_GROUP_CHILDREN_OVERFLOW_MARKER: &str =
    "--moegoe-view-transition-group-children-overflow";
pub const GROUP_ANIMATION_CARRIER_ELEMENT: &str = "moegoe-internal-view-transition-group";
pub const TRANSITION_STYLE_ELEMENT: &str = "moegoe-internal-view-transition-style";
pub const GROUP_STYLE_ELEMENT: &str = "moegoe-internal-view-transition-group-style";
pub const GROUP_CHILDREN_STYLE_ELEMENT: &str =
    "moegoe-internal-view-transition-group-children-style";
pub const INTERNAL_IMAGE_PAIR_PLACEMENT_PREFIX: &str = "--moegoe-view-transition-image-pair-";
pub const NAMED_CAPTURE_ATTRIBUTE: &str = "data-moegoe-view-transition-name";
pub const NAMED_CAPTURE_CLASS_ATTRIBUTE: &str = "data-moegoe-view-transition-class";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewTransitionGroupPseudoSelector {
    authored: String,
    name: String,
}

impl ViewTransitionGroupPseudoSelector {
    pub fn authored(&self) -> &str {
        &self.authored
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[must_use]
pub fn parse_view_transition_group_pseudo_selector(
    value: &str,
) -> Option<ViewTransitionGroupPseudoSelector> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let name = parser
        .parse_entirely(|input| {
            input.expect_colon()?;
            input.expect_colon()?;
            input.expect_function_matching("view-transition-group")?;
            input.parse_nested_block(|inner| {
                let name = CustomIdent::parse(inner, &["none"])?;
                inner.expect_exhausted()?;
                Ok(name.0.as_ref().to_owned())
            })
        })
        .ok()?;
    Some(ViewTransitionGroupPseudoSelector {
        authored: value.to_owned(),
        name,
    })
}

pub fn encode_named_capture_name(name: &str) -> String {
    hex_encode(name)
}

const ROOT_PSEUDO: &[u8] = b"::view-transition";
const OLD_PSEUDO: &[u8] = b"::view-transition-old";
const NEW_PSEUDO: &[u8] = b"::view-transition-new";
const GROUP_PSEUDO: &[u8] = b"::view-transition-group";
const GROUP_CHILDREN_PSEUDO: &[u8] = b"::view-transition-group-children";
const SPECIFICITY_WITNESS: &str = ":not(moegoe-internal-view-transition-root)";

pub fn rewrite_view_transition_root(css: &str) -> Cow<'_, str> {
    if !css
        .as_bytes()
        .windows(ROOT_PSEUDO.len())
        .any(|window| window.eq_ignore_ascii_case(ROOT_PSEUDO))
    {
        return Cow::Borrowed(css);
    }
    let rewritten = rewrite_style_rules(css, &|prelude, body| {
        expand_style_rule(prelude, body).serialization(body)
    });
    if rewritten == css {
        Cow::Borrowed(css)
    } else {
        Cow::Owned(rewritten)
    }
}

pub struct StyleRuleExpansion {
    pub source_selectors: Vec<String>,
    pub generated: String,
}

impl StyleRuleExpansion {
    fn serialization(&self, body: &str) -> String {
        let mut output = String::new();
        for selector in &self.source_selectors {
            let _ = write!(output, "{selector}{{{body}}}");
        }
        output.push_str(&self.generated);
        output
    }
}

pub fn expand_style_rule(prelude: &str, body: &str) -> StyleRuleExpansion {
    let mut source_selectors = vec![prelude.to_owned()];
    if let Some(selectors) = ordinary_selectors_from_mixed_view_transition_list(prelude) {
        source_selectors.push(selectors);
    }
    if let Some(selectors) = project_named_capture_selectors(prelude) {
        source_selectors.push(selectors);
    }
    let mut rule = String::new();
    for (pseudo, declarations) in [
        (
            GROUP_PSEUDO,
            project_ancestor_placement_declarations(body, INTERNAL_GROUP_PLACEMENT_PREFIX),
        ),
        (
            b"::view-transition-image-pair".as_slice(),
            project_ancestor_placement_declarations(body, INTERNAL_IMAGE_PAIR_PLACEMENT_PREFIX),
        ),
    ] {
        append_projected_rule(
            &mut rule,
            project_named_ancestor_capture_selectors(prelude, pseudo),
            declarations,
        );
        append_projected_rule(
            &mut rule,
            project_named_ancestor_capture_selectors(prelude, pseudo),
            project_declarations(body, &["visibility"], None),
        );
    }
    append_projected_rule(
        &mut rule,
        project_named_ancestor_capture_selectors(prelude, GROUP_PSEUDO),
        project_group_border_declarations(body),
    );
    append_projected_rule(
        &mut rule,
        project_named_ancestor_capture_selectors(prelude, b"::view-transition-image-pair"),
        project_declarations(
            body,
            &["opacity"],
            Some(INTERNAL_IMAGE_PAIR_OPACITY_PROPERTY),
        ),
    );
    append_projected_rule(
        &mut rule,
        project_group_animation_carrier_selectors(prelude),
        project_group_carrier_declarations(body),
    );
    append_projected_rule(
        &mut rule,
        project_group_animation_carrier_selectors(prelude),
        project_group_positioned_inset_declarations(body),
    );
    append_projected_rule(
        &mut rule,
        project_group_animation_carrier_selectors(prelude),
        project_group_authored_transform_marker(body),
    );
    append_projected_style_tree_rules(&mut rule, prelude, body);
    append_projected_rule(
        &mut rule,
        project_hidden_root_group_selectors(prelude),
        project_display_declarations(body),
    );
    append_projected_rule(
        &mut rule,
        project_hidden_root_group_selectors(prelude),
        project_declarations(
            body,
            &["opacity"],
            Some(INTERNAL_ROOT_GROUP_OPACITY_PROPERTY),
        ),
    );
    if let (Some(selectors), Some(declarations)) = (
        project_selectors(prelude),
        project_background_declarations(body),
    ) {
        rule.push_str(&selectors);
        rule.push('{');
        rule.push_str(&declarations);
        rule.push('}');
    }
    StyleRuleExpansion {
        source_selectors,
        generated: rule,
    }
}

fn append_projected_style_tree_rules(rule: &mut String, prelude: &str, body: &str) {
    append_projected_rule(
        rule,
        project_group_style_selectors(prelude, GROUP_PSEUDO, GROUP_STYLE_ELEMENT),
        project_declarations(
            body,
            &["background", "background-color", "visibility"],
            None,
        ),
    );
    let group_children_selectors =
        project_group_style_selectors(prelude, GROUP_CHILDREN_PSEUDO, GROUP_CHILDREN_STYLE_ELEMENT);
    append_projected_rule(
        rule,
        group_children_selectors.clone(),
        project_declarations(
            body,
            &[
                "background",
                "background-color",
                "border-block-color",
                "border-block-end-color",
                "border-block-start-color",
                "border-bottom-color",
                "border-color",
                "border-inline-color",
                "border-inline-end-color",
                "border-inline-start-color",
                "border-left-color",
                "border-right-color",
                "border-top-color",
                "overflow",
                "overflow-x",
                "overflow-y",
                "visibility",
            ],
            None,
        ),
    );
    append_projected_rule(
        rule,
        group_children_selectors.clone(),
        project_declarations(body, &["overflow", "overflow-x", "overflow-y"], None)
            .map(|_| format!("{INTERNAL_GROUP_CHILDREN_OVERFLOW_MARKER}:1;")),
    );
    append_projected_rule(
        rule,
        group_children_selectors,
        project_declarations(
            body,
            &[
                "border-block-color",
                "border-block-end-color",
                "border-block-start-color",
                "border-bottom-color",
                "border-color",
                "border-inline-color",
                "border-inline-end-color",
                "border-inline-start-color",
                "border-left-color",
                "border-right-color",
                "border-top-color",
            ],
            None,
        )
        .map(|_| format!("{INTERNAL_GROUP_CHILDREN_BORDER_COLOR_MARKER}:1;")),
    );
    append_projected_rule(
        rule,
        project_transition_style_selectors(prelude),
        project_declarations(
            body,
            &["background", "background-color", "visibility"],
            None,
        ),
    );
}

fn ordinary_selectors_from_mixed_view_transition_list(prelude: &str) -> Option<String> {
    let mut contains_view_transition = false;
    let mut ordinary = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        let selector = prelude[range].trim();
        if selector
            .as_bytes()
            .windows(ROOT_PSEUDO.len())
            .any(|window| window.eq_ignore_ascii_case(ROOT_PSEUDO))
        {
            contains_view_transition = true;
        } else if !selector.is_empty() {
            ordinary.push(selector);
        }
    }
    (contains_view_transition && !ordinary.is_empty()).then(|| ordinary.join(","))
}

fn project_standalone_root_view_transition_rule(css: &str) -> String {
    if !css
        .as_bytes()
        .windows(ROOT_PSEUDO.len())
        .any(|window| window.eq_ignore_ascii_case(ROOT_PSEUDO))
    {
        return String::new();
    }
    rewrite_style_rules_with_opaque_at_rules(
        css,
        &|prelude, body| {
            project_selectors(prelude)
                .map_or_else(String::new, |selectors| format!("{selectors}{{{body}}}"))
        },
        &|prelude| {
            let prelude = prelude.trim_start();
            prelude
                .get(..10)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("@keyframes"))
                || prelude
                    .get(..18)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("@-webkit-keyframes"))
        },
    )
}

#[must_use]
pub fn project_standalone_root_view_transition_root(
    root: &stylo_cssom_model::InternalStylesheetRoot,
) -> stylo_cssom_model::InternalStylesheetRoot {
    let has_standalone_root_rule = root
        .projection_serialization()
        .as_bytes()
        .windows(ROOT_PSEUDO.len())
        .any(|window| window.eq_ignore_ascii_case(ROOT_PSEUDO));
    if !has_standalone_root_rule {
        return stylo_cssom_model::InternalStylesheetRoot::new(
            root.origin(),
            Vec::<stylo_cssom_model::RuleNode>::new(),
        );
    }
    let rules = root
        .rules()
        .iter()
        .filter_map(|rule| {
            if rule.grammar() == stylo_cssom_model::RuleGrammar::Keyframes {
                return Some(vec![rule.clone()]);
            }
            let projection = project_standalone_root_view_transition_rule(&rule.projection_serialization());
            (!projection.is_empty()).then(|| {
                crate::authored_rules::ParsedStylesheet::parse(&projection)
                    .expect("a standalone root view-transition projection must remain a valid stylesheet")
                    .rule_nodes()
                    .to_vec()
            })
        })
        .flatten()
        .collect::<Vec<_>>();
    stylo_cssom_model::InternalStylesheetRoot::new(root.origin(), rules)
}

fn project_hidden_root_group_selectors(prelude: &str) -> Option<String> {
    let mut projected = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        let selector = &prelude[range];
        projected.extend(
            [OLD_PSEUDO, NEW_PSEUDO]
                .into_iter()
                .filter_map(|pseudo| replace_root_group(selector, pseudo)),
        );
    }
    (!projected.is_empty()).then(|| projected.join(","))
}

fn replace_root_group(selector: &str, replacement: &[u8]) -> Option<String> {
    let (cursor, _, open, close) = find_functional_pseudo(selector, &[GROUP_PSEUDO])?;
    if !selector[open + 1..close]
        .trim()
        .eq_ignore_ascii_case("root")
    {
        return None;
    }
    let mut output = String::with_capacity(selector.len());
    output.push_str(&selector[..cursor]);
    output.push_str(":where(:not([");
    output.push_str(NAMED_CAPTURE_ATTRIBUTE);
    output.push_str("]))");
    output.push_str(std::str::from_utf8(replacement).expect("view-transition pseudos are UTF-8"));
    output.push_str("(root)");
    output.push_str(&selector[close + 1..]);
    Some(output)
}

fn project_display_declarations(body: &str) -> Option<String> {
    project_declarations(body, &["display"], None)
}

enum CaptureNameSelector {
    Any,
    ReservedRoot,
    Exact(String),
}

struct NamedCaptureSelector {
    name: CaptureNameSelector,
    classes: Vec<String>,
}

#[derive(Clone, Copy)]
enum NamedPseudoSpecificity {
    Zero,
    Type,
}

impl NamedCaptureSelector {
    fn specificity(&self) -> NamedPseudoSpecificity {
        match (&self.name, self.classes.is_empty()) {
            (CaptureNameSelector::Any, true) => NamedPseudoSpecificity::Zero,
            (
                CaptureNameSelector::Any
                | CaptureNameSelector::ReservedRoot
                | CaptureNameSelector::Exact(_),
                false,
            )
            | (CaptureNameSelector::ReservedRoot | CaptureNameSelector::Exact(_), true) => {
                NamedPseudoSpecificity::Type
            },
        }
    }
}

impl NamedPseudoSpecificity {
    fn witness(self) -> &'static str {
        match self {
            Self::Zero => "",
            Self::Type => SPECIFICITY_WITNESS,
        }
    }
}

fn project_named_capture_selectors(prelude: &str) -> Option<String> {
    let mut projected = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        if let Some(selectors) = project_named_capture_selector(&prelude[range]) {
            projected.extend(selectors);
        }
    }
    (!projected.is_empty()).then(|| projected.join(","))
}

fn project_named_capture_selector(selector: &str) -> Option<Vec<String>> {
    let (cursor, pseudo, open, close) =
        find_functional_pseudo(selector, &[OLD_PSEUDO, NEW_PSEUDO])?;
    let parsed = parse_named_capture_selector(&selector[open + 1..close])?;
    let specificity = parsed.specificity();
    Some(
        capture_host_guards(&parsed)
            .into_iter()
            .map(|guard| {
                project_capture_selector(selector, cursor, pseudo, close, &guard, specificity)
            })
            .collect(),
    )
}

fn project_named_ancestor_capture_selectors(
    prelude: &str,
    ancestor_pseudo: &'static [u8],
) -> Option<String> {
    let mut projected = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        let selector = &prelude[range];
        let Some((cursor, _, open, close)) = find_functional_pseudo(selector, &[ancestor_pseudo])
        else {
            continue;
        };
        let Some(parsed) = parse_named_capture_selector(&selector[open + 1..close]) else {
            continue;
        };
        let specificity = parsed.specificity();
        for guard in capture_host_guards(&parsed) {
            for pseudo in [OLD_PSEUDO, NEW_PSEUDO] {
                projected.push(project_capture_selector(
                    selector,
                    cursor,
                    pseudo,
                    close,
                    &guard,
                    specificity,
                ));
            }
        }
    }
    (!projected.is_empty()).then(|| projected.join(","))
}

fn project_group_animation_carrier_selectors(prelude: &str) -> Option<String> {
    let mut projected = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        let selector = &prelude[range];
        let Some((cursor, _, open, close)) = find_functional_pseudo(selector, &[GROUP_PSEUDO])
        else {
            continue;
        };
        let Some(parsed) = parse_named_capture_selector(&selector[open + 1..close]) else {
            continue;
        };
        let guard = group_animation_carrier_guard(&parsed);
        let specificity = parsed.specificity().witness();
        let prefix = selector[..cursor].trim();
        let suffix = &selector[close + 1..];
        projected.push(if prefix.is_empty() {
            format!("{GROUP_ANIMATION_CARRIER_ELEMENT}{guard}{specificity}{suffix}")
        } else {
            format!("{prefix}>{GROUP_ANIMATION_CARRIER_ELEMENT}{guard}{specificity}{suffix}")
        });
    }
    (!projected.is_empty()).then(|| projected.join(","))
}

fn project_group_style_selectors(
    prelude: &str,
    pseudo: &'static [u8],
    element: &str,
) -> Option<String> {
    let mut projected = Vec::new();
    for range in split_top_level(prelude.as_bytes(), b',') {
        let selector = &prelude[range];
        let Some((cursor, _, open, close)) = find_functional_pseudo(selector, &[pseudo]) else {
            continue;
        };
        let parsed = parse_named_capture_selector(&selector[open + 1..close])?;
        let guard = group_animation_carrier_guard(&parsed);
        let specificity = parsed.specificity().witness();
        let prefix = selector[..cursor].trim();
        let suffix = &selector[close + 1..];
        projected.push(if prefix.is_empty() {
            format!("{element}{guard}{specificity}{suffix}")
        } else {
            format!("{prefix} {element}{guard}{specificity}{suffix}")
        });
    }
    (!projected.is_empty()).then(|| projected.join(","))
}

fn project_transition_style_selectors(prelude: &str) -> Option<String> {
    let projected = split_top_level(prelude.as_bytes(), b',')
        .into_iter()
        .filter_map(|range| project_transition_style_selector(&prelude[range]))
        .collect::<Vec<_>>();
    (!projected.is_empty()).then(|| projected.join(","))
}

fn project_transition_style_selector(selector: &str) -> Option<String> {
    let bytes = selector.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if advance_past_string_or_comment(bytes, &mut cursor) {
            continue;
        }
        let end = cursor + ROOT_PSEUDO.len();
        if end <= bytes.len()
            && bytes[cursor..end].eq_ignore_ascii_case(ROOT_PSEUDO)
            && !bytes
                .get(end)
                .is_some_and(|byte| is_ident_continue(*byte) || *byte == b'(')
        {
            let prefix = selector[..cursor].trim();
            let suffix = &selector[end..];
            return Some(
                if prefix.is_empty() || contains_only_css_whitespace_and_comments(prefix) {
                    format!("{TRANSITION_STYLE_ELEMENT}{suffix}")
                } else {
                    format!("{prefix}>{TRANSITION_STYLE_ELEMENT}{suffix}")
                },
            );
        }
        cursor += 1;
    }
    None
}

fn contains_only_css_whitespace_and_comments(source: &str) -> bool {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        if !matches!(token, Token::WhiteSpace(_) | Token::Comment(_)) {
            return false;
        }
    }
    true
}

fn group_animation_carrier_guard(parsed: &NamedCaptureSelector) -> String {
    let class_guard = capture_class_guard(parsed);
    match &parsed.name {
        CaptureNameSelector::Exact(name) => format!(
            ":where([{NAMED_CAPTURE_ATTRIBUTE}=\"{}\"]{class_guard})",
            encode_named_capture_name(name)
        ),
        CaptureNameSelector::Any => format!(":where([{NAMED_CAPTURE_ATTRIBUTE}]{class_guard})"),
        CaptureNameSelector::ReservedRoot => format!(
            ":where([{NAMED_CAPTURE_ATTRIBUTE}=\"{}\"]{class_guard})",
            encode_named_capture_name("root")
        ),
    }
}

fn capture_host_guards(parsed: &NamedCaptureSelector) -> Vec<String> {
    let class_guard = capture_class_guard(parsed);
    match &parsed.name {
        CaptureNameSelector::Exact(name) => vec![format!(
            ":where([{NAMED_CAPTURE_ATTRIBUTE}=\"{}\"]{class_guard})",
            encode_named_capture_name(name)
        )],
        CaptureNameSelector::ReservedRoot => {
            vec![
                format!(":where(:not([{NAMED_CAPTURE_ATTRIBUTE}]){class_guard})"),
                format!(
                    ":where([{NAMED_CAPTURE_ATTRIBUTE}=\"{}\"]{class_guard})",
                    encode_named_capture_name("root")
                ),
            ]
        },
        CaptureNameSelector::Any => vec![
            format!(":where([{NAMED_CAPTURE_ATTRIBUTE}]{class_guard})"),
            format!(":where(:not([{NAMED_CAPTURE_ATTRIBUTE}]){class_guard})"),
        ],
    }
}

fn capture_class_guard(parsed: &NamedCaptureSelector) -> String {
    let mut guard = String::new();
    for class in &parsed.classes {
        write!(
            guard,
            "[{NAMED_CAPTURE_CLASS_ATTRIBUTE}~=\"{}\"]",
            encode_named_capture_name(class)
        )
        .expect("writing to a String cannot fail");
    }
    guard
}

fn project_capture_selector(
    selector: &str,
    cursor: usize,
    pseudo: &[u8],
    close: usize,
    host_guard: &str,
    specificity: NamedPseudoSpecificity,
) -> String {
    let mut output = String::with_capacity(selector.len() + host_guard.len() + 16);
    output.push_str(&selector[..cursor]);
    output.push_str(host_guard);
    output.push_str(specificity.witness());
    output.push_str(std::str::from_utf8(pseudo).expect("view-transition pseudo tokens are UTF-8"));
    output.push_str("(root)");
    output.push_str(&selector[close + 1..]);
    output
}

fn find_functional_pseudo(
    selector: &str,
    pseudos: &[&'static [u8]],
) -> Option<(usize, &'static [u8], usize, usize)> {
    let bytes = selector.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if advance_past_string_or_comment(bytes, &mut cursor) {
            continue;
        }
        let Some(pseudo) = pseudos.iter().copied().find(|pseudo| {
            cursor + pseudo.len() <= bytes.len()
                && bytes[cursor..cursor + pseudo.len()].eq_ignore_ascii_case(pseudo)
        }) else {
            cursor += 1;
            continue;
        };
        let open = cursor + pseudo.len();
        if bytes.get(open) == Some(&b'(') {
            let close = find_matching_delimiter(bytes, open + 1, b'(', b')')?;
            return Some((cursor, pseudo, open, close));
        }
        cursor += 1;
    }
    None
}

fn parse_named_capture_selector(argument: &str) -> Option<NamedCaptureSelector> {
    let mut input = ParserInput::new(argument.trim());
    let mut parser = Parser::new(&mut input);
    let name = if parser.try_parse(|input| input.expect_delim('*')).is_ok() {
        CaptureNameSelector::Any
    } else if let Ok(ident) = parser.try_parse(|input| CustomIdent::parse(input, &["none"])) {
        if ident.0.as_ref().eq_ignore_ascii_case("root") {
            CaptureNameSelector::ReservedRoot
        } else {
            CaptureNameSelector::Exact(ident.0.as_ref().to_owned())
        }
    } else {
        CaptureNameSelector::Any
    };
    let mut classes = Vec::new();
    while !parser.is_exhausted() {
        if !matches!(
            parser.next_including_whitespace_and_comments().ok()?,
            Token::Delim('.')
        ) {
            return None;
        }
        let location = parser.current_source_location();
        let Token::Ident(ident) = parser
            .next_including_whitespace_and_comments()
            .ok()?
            .clone()
        else {
            return None;
        };
        let class = CustomIdent::from_ident(location, &ident, &["none"]).ok()?;
        classes.push(class.0.as_ref().to_owned());
    }
    if classes.is_empty() && matches!(name, CaptureNameSelector::Any) && argument.trim() != "*" {
        return None;
    }
    Some(NamedCaptureSelector { name, classes })
}

fn project_selectors(prelude: &str) -> Option<String> {
    let projected = split_top_level(prelude.as_bytes(), b',')
        .into_iter()
        .filter_map(|range| strip_root_pseudo(&prelude[range]))
        .collect::<Vec<_>>();
    (!projected.is_empty()).then(|| projected.join(","))
}

fn strip_root_pseudo(selector: &str) -> Option<String> {
    let bytes = selector.as_bytes();
    let mut output = String::with_capacity(selector.len() + SPECIFICITY_WITNESS.len());
    let mut copied = 0;
    let mut cursor = 0;
    let mut found = false;
    while cursor < bytes.len() {
        if advance_past_string_or_comment(bytes, &mut cursor) {
            continue;
        }
        let end = cursor + ROOT_PSEUDO.len();
        if end <= bytes.len()
            && bytes[cursor..end].eq_ignore_ascii_case(ROOT_PSEUDO)
            && !bytes
                .get(end)
                .is_some_and(|byte| is_ident_continue(*byte) || *byte == b'(')
        {
            output.push_str(&selector[copied..cursor]);
            output.push_str(SPECIFICITY_WITNESS);
            copied = end;
            cursor = end;
            found = true;
        } else {
            cursor += 1;
        }
    }
    found.then(|| {
        output.push_str(&selector[copied..]);
        output
    })
}

fn append_projected_rule(
    rule: &mut String,
    selectors: Option<String>,
    declarations: Option<String>,
) {
    let (Some(selectors), Some(declarations)) = (selectors, declarations) else {
        return;
    };
    rule.push_str(&selectors);
    rule.push('{');
    rule.push_str(&declarations);
    rule.push('}');
}

fn project_background_declarations(body: &str) -> Option<String> {
    project_declarations(
        body,
        &["background", "background-color"],
        Some(INTERNAL_BACKGROUND_PROPERTY),
    )
}

fn project_ancestor_placement_declarations(body: &str, prefix: &str) -> Option<String> {
    let mut output = String::new();
    for property in ["top", "right", "bottom", "left", "transform"] {
        let replacement = format!("{prefix}{property}");
        if let Some(declaration) = project_declarations(body, &[property], Some(&replacement)) {
            output.push_str(&declaration);
        }
    }
    (!output.is_empty()).then_some(output)
}

fn project_group_carrier_declarations(body: &str) -> Option<String> {
    project_declarations(
        body,
        &[
            "animation",
            "animation-composition",
            "animation-delay",
            "animation-direction",
            "animation-duration",
            "animation-fill-mode",
            "animation-iteration-count",
            "animation-name",
            "animation-play-state",
            "animation-range",
            "animation-range-end",
            "animation-range-start",
            "animation-timeline",
            "animation-timing-function",
            "height",
            "width",
        ],
        None,
    )
}

fn project_group_positioned_inset_declarations(body: &str) -> Option<String> {
    let mut output = String::new();
    for property in ["top", "right", "bottom", "left"] {
        let replacement = format!("{INTERNAL_GROUP_PLACEMENT_PREFIX}{property}");
        if let Some(declaration) = project_declarations(body, &[property], Some(&replacement)) {
            output.push_str(&declaration);
        }
    }
    (!output.is_empty()).then_some(output)
}

fn project_group_authored_transform_marker(body: &str) -> Option<String> {
    project_declarations(body, &["transform"], None)
        .map(|_| format!("{INTERNAL_GROUP_AUTHORED_TRANSFORM_MARKER}:1;"))
}

fn project_group_border_declarations(body: &str) -> Option<String> {
    project_declarations(
        body,
        &[
            "border",
            "border-block",
            "border-block-color",
            "border-block-end",
            "border-block-end-color",
            "border-block-end-style",
            "border-block-end-width",
            "border-block-start",
            "border-block-start-color",
            "border-block-start-style",
            "border-block-start-width",
            "border-block-style",
            "border-block-width",
            "border-bottom",
            "border-bottom-color",
            "border-bottom-left-radius",
            "border-bottom-right-radius",
            "border-bottom-style",
            "border-bottom-width",
            "border-color",
            "border-inline",
            "border-inline-color",
            "border-inline-end",
            "border-inline-end-color",
            "border-inline-end-style",
            "border-inline-end-width",
            "border-inline-start",
            "border-inline-start-color",
            "border-inline-start-style",
            "border-inline-start-width",
            "border-inline-style",
            "border-inline-width",
            "border-left",
            "border-left-color",
            "border-left-style",
            "border-left-width",
            "border-radius",
            "border-right",
            "border-right-color",
            "border-right-style",
            "border-right-width",
            "border-start-end-radius",
            "border-start-start-radius",
            "border-style",
            "border-top",
            "border-top-color",
            "border-top-left-radius",
            "border-top-right-radius",
            "border-top-style",
            "border-top-width",
            "border-width",
        ],
        None,
    )
}

fn project_declarations(
    body: &str,
    properties: &[&str],
    replacement: Option<&str>,
) -> Option<String> {
    let mut output = String::new();
    for range in split_top_level(body.as_bytes(), b';') {
        let declaration = &body[range];
        let Some(colon) = declaration.find(':') else {
            continue;
        };
        let property = declaration[..colon].trim();
        if !properties
            .iter()
            .any(|candidate| property.eq_ignore_ascii_case(candidate))
        {
            continue;
        }
        match replacement {
            Some(replacement) => {
                output.push_str(replacement);
                output.push_str(&declaration[colon..]);
            },
            None => output.push_str(declaration),
        }
        output.push(';');
    }
    (!output.is_empty()).then_some(output)
}

#[cfg(test)]
mod tests {
    use super::{
        GROUP_CHILDREN_STYLE_ELEMENT, GROUP_STYLE_ELEMENT, INTERNAL_BACKGROUND_PROPERTY,
        INTERNAL_IMAGE_PAIR_OPACITY_PROPERTY, SPECIFICITY_WITNESS, TRANSITION_STYLE_ELEMENT,
        parse_view_transition_group_pseudo_selector, project_named_capture_selector,
        project_standalone_root_view_transition_root, rewrite_view_transition_root,
    };
    use crate::selector_query::selector_specificity;

    #[test]
    fn parses_authored_view_transition_group_effect_targets() {
        for selector in [
            "::view-transition-group( first)",
            "::view-transition-group(first)",
            "::view-transition-group( first",
            "::view-transition-group(      first )",
            "::view-transition-group(first )",
            "::view-transition-group(first",
        ] {
            let parsed = parse_view_transition_group_pseudo_selector(selector)
                .expect("the supported pseudo-element selector must parse");
            assert_eq!(parsed.name(), "first");
            assert_eq!(parsed.authored(), selector);
        }
    }

    #[test]
    fn rejects_non_group_or_non_specific_effect_targets() {
        for selector in [
            "::before",
            "::view-transition-group(*)",
            "::view-transition-group(none)",
            "::view-transition-group(first) trailing",
            ":view-transition-group(first)",
        ] {
            assert!(parse_view_transition_group_pseudo_selector(selector).is_none());
        }
    }

    #[test]
    fn mirrors_only_non_functional_root_pseudo_backgrounds() {
        let css = concat!(
            ":root::view-transition{background:blue;animation:hold 1s}",
            "html::VIEW-TRANSITION { background-color: rgb(1, 2, 3) !important }",
            "::view-transition-old(root){background:red}",
            ".literal{content:'::view-transition{background:pink}'}",
        );
        let rewritten = rewrite_view_transition_root(css);

        assert!(rewritten.contains(&format!(
            ":root:not(moegoe-internal-view-transition-root){{{INTERNAL_BACKGROUND_PROPERTY}:blue;}}"
        )));
        assert!(rewritten.contains(&format!(
            "html:not(moegoe-internal-view-transition-root) {{{INTERNAL_BACKGROUND_PROPERTY}: rgb(1, 2, 3) !important ;}}"
        )));
        assert_eq!(rewritten.matches(INTERNAL_BACKGROUND_PROPERTY).count(), 2);
    }

    #[test]
    fn keeps_conditional_rule_ownership() {
        let rewritten =
            rewrite_view_transition_root("@media print{:root::view-transition{background:blue}}");

        assert!(rewritten.contains(&format!(
            "@media print{{:root::view-transition{{background:blue}}:root>{TRANSITION_STYLE_ELEMENT}{{background:blue;}}:root:not(moegoe-internal-view-transition-root){{{INTERNAL_BACKGROUND_PROPERTY}:blue;}}}}"
        )));
    }

    #[test]
    fn projects_only_standalone_root_rules_and_their_keyframes() {
        let css = concat!(
            "html{animation:unrelated 1s}",
            "@keyframes hold{from{opacity:1}to{opacity:1}}",
            "@media screen{html::view-transition{animation:hold 300s;background:blue}}",
            "html::view-transition-old(root){animation:none}",
        );
        let parsed = crate::ParsedStylesheet::parse(css)
            .expect("the standalone projection fixture must parse");
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );
        let projected =
            project_standalone_root_view_transition_root(&root).projection_serialization();

        assert!(!projected.contains("unrelated"));
        assert!(projected.contains("@keyframes hold{from{opacity:1}to{opacity:1}}"));
        assert!(projected.contains(concat!(
            "@media screen{html:not(moegoe-internal-view-transition-root)",
            "{animation:hold 300s;background:blue}}",
        )));
        assert!(!projected.contains("view-transition-old"));
    }

    #[test]
    fn projects_named_capture_pseudos_to_guarded_root_pseudos() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-old(card){opacity:1}",
            "html::view-transition-new(*){opacity:0}",
            "@media screen{html::view-transition-old(Card){opacity:.5}}",
        ));

        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name=\"63617264\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-old(root){opacity:1}",
        )));
        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name])",
            "::view-transition-new(root),",
            "html:where(:not([data-moegoe-view-transition-name]))",
            "::view-transition-new(root){opacity:0}",
        )));
        assert!(rewritten.contains(concat!(
            "@media screen{html::view-transition-old(Card){opacity:.5}",
            "html:where([data-moegoe-view-transition-name=\"43617264\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-old(root){opacity:.5}}",
        )));
    }

    #[test]
    fn projects_wildcard_capture_pseudos_to_named_and_reserved_root_hosts() {
        let rewritten = rewrite_view_transition_root("html::view-transition-new(*){opacity:1}");

        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name])",
            "::view-transition-new(root),",
            "html:where(:not([data-moegoe-view-transition-name]))",
            "::view-transition-new(root){opacity:1}",
        )));
    }

    #[test]
    fn projects_capture_class_subsets_with_the_name_guard() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-old(card.cls.some-div){left:100px}",
            "html::view-transition-new(*.cls.some-div){left:100px}",
            "html::view-transition-old(.cls){left:100px}",
        ));

        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name=\"63617264\"]",
            "[data-moegoe-view-transition-class~=\"636c73\"]",
            "[data-moegoe-view-transition-class~=\"736f6d652d646976\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-old(root){left:100px}",
        )));
        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name]",
            "[data-moegoe-view-transition-class~=\"636c73\"]",
            "[data-moegoe-view-transition-class~=\"736f6d652d646976\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-new(root),",
            "html:where(:not([data-moegoe-view-transition-name])",
            "[data-moegoe-view-transition-class~=\"636c73\"]",
            "[data-moegoe-view-transition-class~=\"736f6d652d646976\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-new(root){left:100px}",
        )));
        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name]",
            "[data-moegoe-view-transition-class~=\"636c73\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-old(root),",
            "html:where(:not([data-moegoe-view-transition-name])",
            "[data-moegoe-view-transition-class~=\"636c73\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-old(root){left:100px}",
        )));
    }

    #[test]
    fn projected_named_pseudos_retain_their_standard_specificity_relation() {
        let projected_specificity = |argument: &str| {
            let selector = format!("html::view-transition-old({argument})");
            let projected = project_named_capture_selector(&selector)
                .expect("the named pseudo must project")
                .into_iter()
                .next()
                .expect("the projection must contain a named host");
            selector_specificity(&projected).expect("the projected selector must parse")
        };
        let wildcard = projected_specificity("*");
        let named = projected_specificity("shared");
        let one_class = projected_specificity("*.first");
        let two_classes = projected_specificity("*.first.second");

        assert_eq!(named, wildcard + 1);
        assert_eq!(one_class, named);
        assert_eq!(two_classes, named);
    }

    #[test]
    fn projects_reserved_root_capture_classes_only_to_the_reserved_host() {
        let rewritten =
            rewrite_view_transition_root("html::view-transition-new(root.cls){left:100px}");

        assert!(rewritten.contains(concat!(
            "html:where(:not([data-moegoe-view-transition-name])",
            "[data-moegoe-view-transition-class~=\"636c73\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-new(root),",
            "html:where([data-moegoe-view-transition-name=\"726f6f74\"]",
            "[data-moegoe-view-transition-class~=\"636c73\"])",
            ":not(moegoe-internal-view-transition-root)",
            "::view-transition-new(root){left:100px}",
        )));
        assert_eq!(
            rewritten
                .matches("data-moegoe-view-transition-class")
                .count(),
            2
        );
    }

    #[test]
    fn projects_class_qualified_ancestor_placement_as_distinct_capture_stages() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-group(card.cls){left:100px}",
            "html::view-transition-image-pair(card.cls){transform:translateX(100px)}",
        ));

        let guard = concat!(
            "html:where([data-moegoe-view-transition-name=\"63617264\"]",
            "[data-moegoe-view-transition-class~=\"636c73\"])"
        );
        assert!(rewritten.contains(&format!(
            "{guard}{SPECIFICITY_WITNESS}::view-transition-old(root),{guard}{SPECIFICITY_WITNESS}::view-transition-new(root){{",
        )));
        assert!(rewritten.contains("--moegoe-view-transition-group-left:100px"));
        assert!(
            rewritten.contains("--moegoe-view-transition-image-pair-transform:translateX(100px)")
        );
    }

    #[test]
    fn projects_group_animation_timing_to_isolated_carrier() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-group(item){",
            "animation-duration:250ms;",
            "animation-timing-function:steps(2,start);",
            "animation-play-state:paused}",
        ));

        assert!(rewritten.contains(concat!(
            "html>moegoe-internal-view-transition-group",
            ":where([data-moegoe-view-transition-name=\"6974656d\"])",
            ":not(moegoe-internal-view-transition-root){",
            "animation-duration:250ms;",
            "animation-timing-function:steps(2,start);",
            "animation-play-state:paused;}",
        )));
    }

    #[test]
    fn projects_group_and_group_children_backgrounds_to_the_style_tree() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-group(parent){background:green}",
            "::view-transition-group-children(parent){background:inherit}",
            "::view-transition-group(child){background:inherit}",
        ));

        assert!(rewritten.contains(&format!(
            "html {GROUP_STYLE_ELEMENT}:where([data-moegoe-view-transition-name=\"706172656e74\"]){SPECIFICITY_WITNESS}{{background:green;}}",
        )));
        assert!(rewritten.contains(&format!(
            "{GROUP_CHILDREN_STYLE_ELEMENT}:where([data-moegoe-view-transition-name=\"706172656e74\"]){SPECIFICITY_WITNESS}{{background:inherit;}}",
        )));
        assert!(rewritten.contains(&format!(
            "{GROUP_STYLE_ELEMENT}:where([data-moegoe-view-transition-name=\"6368696c64\"]){SPECIFICITY_WITNESS}{{background:inherit;}}",
        )));
    }

    #[test]
    fn projects_group_children_clip_and_border_paint_to_the_style_tree() {
        let rewritten = rewrite_view_transition_root(
            "::view-transition-group-children(parent){overflow:clip;border-color:green}",
        );

        assert!(rewritten.contains(&format!(
            "{GROUP_CHILDREN_STYLE_ELEMENT}:where([data-moegoe-view-transition-name=\"706172656e74\"]){SPECIFICITY_WITNESS}{{overflow:clip;border-color:green;}}",
        )));
    }

    #[test]
    fn projects_transition_background_to_the_generated_style_parent() {
        let rewritten = rewrite_view_transition_root(concat!(
            "::view-transition{background:red}",
            "html.ready::view-transition{background:green}",
        ));

        assert!(rewritten.contains(&format!("{TRANSITION_STYLE_ELEMENT}{{background:red;}}")));
        assert!(rewritten.contains(&format!(
            "html.ready>{TRANSITION_STYLE_ELEMENT}{{background:green;}}"
        )));
    }

    #[test]
    fn leading_comment_is_whitespace_for_the_transition_style_parent() {
        let rewritten = rewrite_view_transition_root(
            "/* The generated tree remains visible. */\n::view-transition{visibility:visible}",
        );

        assert!(rewritten.contains(&format!(
            "{TRANSITION_STYLE_ELEMENT}{{visibility:visible;}}"
        )));
        assert!(!rewritten.contains("*/>"));
    }

    #[test]
    fn preserves_ordinary_selectors_from_mixed_view_transition_selector_lists() {
        let rewritten = rewrite_view_transition_root(concat!(
            "::view-transition,::view-transition-group(*),div{",
            "position:absolute;inset:0;background:red}",
        ));

        assert_eq!(
            rewritten
                .matches("position:absolute;inset:0;background:red")
                .count(),
            2,
            "{rewritten}",
        );
    }

    #[test]
    fn projects_group_dimensions_to_the_geometry_carrier() {
        let rewritten = rewrite_view_transition_root(concat!(
            "html::view-transition-group(item){",
            "animation:unset;width:50px;height:100px}",
        ));

        assert!(rewritten.contains(concat!(
            "html>moegoe-internal-view-transition-group",
            ":where([data-moegoe-view-transition-name=\"6974656d\"])",
            ":not(moegoe-internal-view-transition-root){",
            "animation:unset;",
            "width:50px;",
            "height:100px;}",
        )));
    }

    #[test]
    fn projects_group_border_to_each_capture_phase() {
        let rewritten = rewrite_view_transition_root(
            "html::view-transition-group(item){border:2px solid black;border-radius:3px}",
        );

        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name=\"6974656d\"])",
            ":not(moegoe-internal-view-transition-root)::view-transition-old(root),",
            "html:where([data-moegoe-view-transition-name=\"6974656d\"])",
            ":not(moegoe-internal-view-transition-root)::view-transition-new(root)",
            "{border:2px solid black;border-radius:3px;}",
        )));
    }

    #[test]
    fn projects_wildcard_group_pause_to_each_named_carrier() {
        let rewritten = rewrite_view_transition_root(concat!(
            "::view-transition-group(*),::view-transition-new(*),",
            "::view-transition-old(*),::view-transition-image-pair(*){animation-play-state:paused}",
        ));

        assert!(rewritten.contains(concat!(
            "moegoe-internal-view-transition-group",
            ":where([data-moegoe-view-transition-name]){animation-play-state:paused;}",
        )));
    }

    #[test]
    fn projects_reserved_root_group_timing_to_an_isolated_carrier() {
        let rewritten = rewrite_view_transition_root(
            "html::view-transition-group(root){animation-duration:500s;animation-play-state:paused}",
        );

        assert!(
            rewritten.contains(concat!(
                "html>moegoe-internal-view-transition-group",
                ":where([data-moegoe-view-transition-name=\"726f6f74\"])",
                ":not(moegoe-internal-view-transition-root){",
                "animation-duration:500s;",
                "animation-play-state:paused;}",
            )),
            "{rewritten}"
        );
    }

    #[test]
    fn rejects_whitespace_inside_capture_class_selectors() {
        for selector in ["card .cls", "card. cls", "*.cls .some-div"] {
            let css = format!("html::view-transition-old({selector}){{left:100px}}");
            let rewritten = rewrite_view_transition_root(&css);

            assert_eq!(rewritten, css);
        }
    }

    #[test]
    fn projects_reserved_root_group_display_to_both_capture_children() {
        let rewritten = rewrite_view_transition_root(
            "html::view-transition-group(root){display:none;opacity:0;animation:unset}",
        );

        assert!(rewritten.contains(concat!(
            "html:where(:not([data-moegoe-view-transition-name]))::view-transition-old(root),",
            "html:where(:not([data-moegoe-view-transition-name]))::view-transition-new(root)",
            "{display:none;}",
        )));
        assert!(rewritten.contains(concat!(
            "html:where(:not([data-moegoe-view-transition-name]))::view-transition-old(root),",
            "html:where(:not([data-moegoe-view-transition-name]))::view-transition-new(root)",
            "{--moegoe-view-transition-root-group-opacity:0;}",
        )));
        assert_eq!(rewritten.matches("animation:unset").count(), 2);
    }

    #[test]
    fn projects_named_image_pair_opacity_to_both_capture_children() {
        let rewritten = rewrite_view_transition_root(
            "html::view-transition-image-pair(hidden){animation:unset;opacity:0}",
        );

        assert!(rewritten.contains(&format!(
            "html:where([data-moegoe-view-transition-name=\"68696464656e\"]):not(moegoe-internal-view-transition-root)::view-transition-old(root),\
             html:where([data-moegoe-view-transition-name=\"68696464656e\"]):not(moegoe-internal-view-transition-root)::view-transition-new(root)\
             {{{INTERNAL_IMAGE_PAIR_OPACITY_PROPERTY}:0;}}"
        )));
    }

    #[test]
    fn projects_named_group_visibility_to_both_capture_children() {
        let rewritten =
            rewrite_view_transition_root("html::view-transition-group(hidden){visibility:hidden}");

        assert!(rewritten.contains(concat!(
            "html:where([data-moegoe-view-transition-name=\"68696464656e\"]):not(moegoe-internal-view-transition-root)::view-transition-old(root),",
            "html:where([data-moegoe-view-transition-name=\"68696464656e\"]):not(moegoe-internal-view-transition-root)::view-transition-new(root)",
            "{visibility:hidden;}"
        )));
    }
}
