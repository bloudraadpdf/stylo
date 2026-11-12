/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::test_support::{pref_lock, test_device, test_url_data};
use selectors::matching::QuirksMode;

fn parse(css: &str) -> Result<VariableValue, ()> {
    let mut input = ParserInput::new(css);
    Parser::new(&mut input)
        .parse_entirely(|input| {
            VariableValue::parse(input, &test_url_data(), &Namespaces::default())
        })
        .map_err(|_| ())
}

fn substituted(css: &str) -> Result<String, ()> {
    substituted_with_mode(css, EnvironmentResolutionMode::TreatAsMissing)
}

fn substituted_with_mode(css: &str, mode: EnvironmentResolutionMode) -> Result<String, ()> {
    let value = parse(css)?;
    let stylist = Stylist::new(test_device(), QuirksMode::NoQuirks);
    computed::Context::for_media_query_evaluation(
        stylist.device(),
        QuirksMode::NoQuirks,
        |context| {
            substitute_with_environment_resolution(
                &value,
                &ComputedCustomProperties::default(),
                &stylist,
                context,
                CascadeLevel::same_tree_author_normal(),
                mode,
                &mut AttributeTracker::new_dummy(),
            )
            .map(|value| value.css.into_owned())
        },
    )
}

#[test]
fn braced_env_fallbacks_substitute_only_their_contents() {
    let _guard = pref_lock().lock().unwrap();
    for (css, expected) in [
        ("env(missing-name, {rgb(1, 2, 3)})", "rgb(1, 2, 3)"),
        ("env(missing-name, { rgb(1, 2, 3) })", "rgb(1, 2, 3) "),
        ("env(missing-name, {rgb(1, 2, 3)} )", "rgb(1, 2, 3)"),
        (
            "env(missing-name, {Arial, sans-serif})",
            "Arial, sans-serif",
        ),
        ("[env(missing-name, {})]", "[]"),
        ("env(missing-name, {env(other-name, {30px})})", "30px"),
    ] {
        assert_eq!(substituted(css).as_deref(), Ok(expected), "{css}");
    }
}

#[test]
fn braced_env_fallbacks_reject_tokens_outside_the_block() {
    let _guard = pref_lock().lock().unwrap();
    for css in [
        "env(missing-name, {rgb(1, 2, 3)} rgb(4, 5, 6))",
        "env(missing-name, {Arial}, sans-serif)",
        "env(safe-area-inset-top, {1px} 2px)",
    ] {
        assert!(parse(css).is_err(), "{css}");
    }
}

#[test]
fn env_fallback_bounds_keep_commas_whitespace_and_eof_recovery() {
    let _guard = pref_lock().lock().unwrap();
    for (css, expected) in [
        ("env(missing-name, Arial, sans-serif)", "Arial, sans-serif"),
        ("env(missing-name, 30px   )", "30px   "),
        (
            "env(missing-name, 30px /* comment */)",
            "30px /* comment */",
        ),
        ("[env(missing-name,)]", "[]"),
        ("env(missing-name, \"fallback", "\"fallback\""),
        ("env(missing-name, {\"fallback", "\"fallback\""),
        ("env(missing-name, {rgb(1,2,3", "rgb(1,2,3)"),
        (
            "env(missing-name, {env(other-name, {rgb(1,2,3",
            "rgb(1,2,3)",
        ),
    ] {
        assert_eq!(substituted(css).as_deref(), Ok(expected), "{css}");
    }
}

#[test]
fn environment_argument_grammar_is_deferred_but_empty_or_mixed_blocks_are_rejected() {
    let _guard = pref_lock().lock().unwrap();
    for name in ["foo", "0", ".1", "-1", "foo bar", "fn()", "initial"] {
        assert!(parse(&format!("env({name}, red)")).is_ok(), "{name}");
    }
    for css in [
        "env()",
        "env({})",
        "env({}, red)",
        "env(, red)",
        "env({foo} bar, red)",
        "env(foo {bar}, red)",
    ] {
        assert!(parse(css).is_err(), "{css}");
    }
}

#[test]
fn invalid_standard_environment_grammar_does_not_use_the_fallback() {
    let _guard = pref_lock().lock().unwrap();
    for name in [
        "0",
        ".1",
        "-1",
        "foo bar",
        "fn()",
        "initial",
        "inherit",
        "unset",
        "revert",
        "revert-layer",
        "safe-area-inset-top -1",
        "safe-area-inset-top 1.5",
    ] {
        assert!(substituted(&format!("env({name}, red)")).is_err(), "{name}");
    }
}

#[test]
fn environment_lookup_distinguishes_unknown_names_and_scalar_index_mismatches() {
    let _guard = pref_lock().lock().unwrap();
    for (css, expected) in [
        ("env(safe-area-inset-top, 30px)", "0.0px"),
        ("env({safe-area-inset-top}, 30px)", "0.0px"),
        ("env({var(--missing,safe-area-inset-top", "0.0px"),
        ("env(safe-area-inset-top 0, 30px)", "30px"),
        ("env(missing-name, 30px)", "30px"),
        ("env(missing-name 0 1, 30px)", "30px"),
    ] {
        assert_eq!(
            substituted_with_mode(css, EnvironmentResolutionMode::ResolveLiveEnvironment)
                .as_deref(),
            Ok(expected),
            "{css}"
        );
    }
}

#[test]
fn environment_fallbacks_do_not_add_eager_custom_property_dependencies() {
    let _guard = pref_lock().lock().unwrap();
    for (css, expected) in [
        ("env(safe-area-inset-top, var(--fallback))", None),
        ("env(var(--name), var(--fallback))", Some("name")),
    ] {
        let value = parse(css).unwrap();
        let dependencies = value
            .references
            .eager_variable_names()
            .map(|name| name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(
            dependencies,
            expected.into_iter().collect::<Vec<_>>(),
            "{css}"
        );
    }
}
