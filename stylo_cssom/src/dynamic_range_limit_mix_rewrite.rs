use std::borrow::Cow;

use crate::css_scan::{find_matching_close_paren, is_ident_continue, utf8_char_width};

const DRLM_OPEN: &[u8] = b"dynamic-range-limit-mix(";

pub fn rewrite_dynamic_range_limit_mix(css: &str) -> Cow<'_, str> {
    if !contains_drlm(css) {
        return Cow::Borrowed(css);
    }
    Cow::Owned(rewrite_owned(css))
}

fn contains_drlm(css: &str) -> bool {
    crate::css_scan::contains_any_ascii_ci(css, &[DRLM_OPEN])
}

fn rewrite_owned(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if i + DRLM_OPEN.len() <= bytes.len()
            && bytes[i..i + DRLM_OPEN.len()].eq_ignore_ascii_case(DRLM_OPEN)
        {
            let at_token_boundary = i == 0 || !is_ident_continue(bytes[i - 1]);
            if at_token_boundary {
                if let Some(resolved) = parse_and_resolve(bytes, i) {
                    out.push_str(resolved.keyword);
                    i = resolved.end;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
                continue;
            }
        }

        let char_len = utf8_char_width(bytes[i]);
        out.push_str(&css[i..i + char_len]);
        i += char_len;
    }

    out
}

struct Resolved {
    keyword: &'static str,
    end: usize,
}

fn parse_and_resolve(bytes: &[u8], start: usize) -> Option<Resolved> {
    let after_open = start + DRLM_OPEN.len();
    let close = find_matching_close_paren(bytes, after_open)?;
    let body = std::str::from_utf8(&bytes[after_open..close]).ok()?;

    let mut parts = body.splitn(3, ',');
    let a_raw = parts.next()?.trim();
    let b_raw = parts.next()?.trim();
    let p_raw = parts.next()?.trim();
    if parts.next().is_some() {
        return None;
    }

    let a = parse_drl_keyword(a_raw)?;
    let b = parse_drl_keyword(b_raw)?;
    let p = parse_percentage(p_raw)?;

    let interp = a * (1.0 - p) + b * p;
    let keyword = nearest_keyword(interp);
    Some(Resolved {
        keyword,
        end: close + 1,
    })
}

fn parse_drl_keyword(token: &str) -> Option<f32> {
    match token.to_ascii_lowercase().as_str() {
        "standard" => Some(0.0),
        "constrained-high" => Some(0.5),
        "high" => Some(1.0),
        _ => None,
    }
}

fn parse_percentage(token: &str) -> Option<f32> {
    let stripped = token.strip_suffix('%')?.trim();
    let raw: f32 = stripped.parse().ok()?;
    Some((raw / 100.0).clamp(0.0, 1.0))
}

fn nearest_keyword(value: f32) -> &'static str {
    const STOPS: [(f32, &str); 3] = [(0.0, "standard"), (0.5, "constrained-high"), (1.0, "high")];
    let mut best = STOPS[1];
    let mut best_dist = (value - STOPS[1].0).abs();
    for &(stop, name) in &STOPS {
        let dist = (value - stop).abs();
        if dist < best_dist {
            best_dist = dist;
            best = (stop, name);
        }
    }
    best.1
}
