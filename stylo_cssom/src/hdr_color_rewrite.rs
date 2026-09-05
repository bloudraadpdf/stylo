use std::borrow::Cow;

use crate::css_scan::{
    current_property_name, find_matching_close_paren, is_css_whitespace, is_ident_continue,
    utf8_char_width,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdrTransfer {
    Pq,

    Hlg,

    Linear,
}

impl HdrTransfer {
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Pq => "pq",
            Self::Hlg => "hlg",
            Self::Linear => "linear",
        }
    }

    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker.trim() {
            "pq" => Some(Self::Pq),
            "hlg" => Some(Self::Hlg),
            "linear" => Some(Self::Linear),
            _ => None,
        }
    }
}

pub const HDR_MARKER_PREFIX: &str = "--__moegoe_hdr_";

const COLOR_OPEN: &[u8] = b"color(";

const HDR_COLOR_OPEN: &[u8] = b"hdr-color(";

pub fn marker_property_name(longhand: &str) -> String {
    let mut out = String::with_capacity(HDR_MARKER_PREFIX.len() + longhand.len());
    out.push_str(HDR_MARKER_PREFIX);
    out.push_str(longhand);
    out
}

pub fn rewrite_hdr_color_calls(css: &str) -> Cow<'_, str> {
    if !contains_hdr_marker(css) {
        return Cow::Borrowed(css);
    }
    Cow::Owned(rewrite_owned(css))
}

fn contains_hdr_marker(css: &str) -> bool {
    let bytes = css.as_bytes();
    let has_color = bytes
        .windows(b"color(".len())
        .any(|w| w.eq_ignore_ascii_case(b"color("));
    if !has_color {
        return false;
    }
    bytes
        .windows(b"rec2100-".len())
        .any(|w| w.eq_ignore_ascii_case(b"rec2100-"))
}

fn hdr_call_opener_len(bytes: &[u8], index: usize) -> Option<usize> {
    let matches = |opener: &[u8]| {
        index + opener.len() <= bytes.len()
            && bytes[index..index + opener.len()].eq_ignore_ascii_case(opener)
    };
    if matches(HDR_COLOR_OPEN) {
        Some(HDR_COLOR_OPEN.len())
    } else if matches(COLOR_OPEN) {
        Some(COLOR_OPEN.len())
    } else {
        None
    }
}

fn rewrite_owned(css: &str) -> String {
    let mut out = String::with_capacity(css.len() + 64);
    let bytes = css.as_bytes();
    let mut i = 0;

    let mut pending_markers: Vec<(usize, String)> = Vec::new();

    while i < bytes.len() {
        if bytes[i] == b';' || bytes[i] == b'}' {
            for (_idx, marker) in pending_markers.drain(..) {
                out.push_str(&marker);
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if let Some(opener_len) = hdr_call_opener_len(bytes, i)
            && (i == 0 || !is_ident_continue(bytes[i - 1]))
        {
            if let Some(parsed) = parse_hdr_color_call(bytes, i, opener_len) {
                let longhand = current_property_name(&out);
                out.push_str("color(rec2020 ");
                out.push_str(parsed.components);
                out.push(')');
                if let Some(name) = longhand {
                    let marker = format!(
                        "; {}: {}",
                        marker_property_name(&name),
                        parsed.transfer.marker()
                    );
                    pending_markers.push((out.len(), marker));
                }
                i = parsed.end;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
            continue;
        }

        let char_len = utf8_char_width(bytes[i]);
        out.push_str(&css[i..i + char_len]);
        i += char_len;
    }

    for (_idx, marker) in pending_markers.drain(..) {
        out.push_str(&marker);
    }

    out
}

struct ParsedHdrCall<'a> {
    transfer: HdrTransfer,
    components: &'a str,
    end: usize,
}

fn parse_hdr_color_call(
    bytes: &[u8],
    start: usize,
    fn_token_len: usize,
) -> Option<ParsedHdrCall<'_>> {
    let after_open = start + fn_token_len;

    let close = find_matching_close_paren(bytes, after_open)?;
    let body = &bytes[after_open..close];

    let mut k = 0;
    while k < body.len() && is_css_whitespace(body[k]) {
        k += 1;
    }
    let kw_start = k;
    while k < body.len() && is_ident_continue(body[k]) {
        k += 1;
    }
    let kw = &body[kw_start..k];
    let transfer = match kw {
        b if b.eq_ignore_ascii_case(b"rec2100-pq") => HdrTransfer::Pq,
        b if b.eq_ignore_ascii_case(b"rec2100-hlg") => HdrTransfer::Hlg,
        b if b.eq_ignore_ascii_case(b"rec2100-linear") => HdrTransfer::Linear,
        _ => return None,
    };

    while k < body.len() && is_css_whitespace(body[k]) {
        k += 1;
    }
    if k >= body.len() {
        return None;
    }
    let components_bytes = &body[k..];

    let components = std::str::from_utf8(components_bytes).ok()?;
    Some(ParsedHdrCall {
        transfer,
        components,
        end: close + 1,
    })
}
