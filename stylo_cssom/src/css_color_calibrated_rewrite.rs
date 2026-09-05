use std::{borrow::Cow, fmt::Write as _};

use crate::css_scan::{
    current_property_name, find_matching_close_paren, is_ident_continue, parse_component_as_f32,
    utf8_char_width,
};

pub const CALIBRATED_MARKER_PREFIX: &str = "--__moegoe_calibrated_";

pub const D50_WHITE_POINT: [f32; 3] = [0.9642, 1.0, 0.8249];

pub const D65_WHITE_POINT: [f32; 3] = [0.9505, 1.0, 1.089];

pub const ILLUMINANT_E_WHITE_POINT: [f32; 3] = [1.0, 1.0, 1.0];

pub const ILLUMINANT_C_WHITE_POINT: [f32; 3] = [0.9807, 1.0, 1.1822];

pub fn marker_property_name(longhand: &str) -> String {
    let mut out = String::with_capacity(CALIBRATED_MARKER_PREFIX.len() + longhand.len());
    out.push_str(CALIBRATED_MARKER_PREFIX);
    out.push_str(longhand);
    out
}

pub fn rewrite_calibrated_color_calls(css: &str) -> Cow<'_, str> {
    if !contains_calibrated_call(css) {
        return Cow::Borrowed(css);
    }
    Cow::Owned(rewrite_owned(css))
}

const CAL_RGB_OPEN: &[u8] = b"color(--bd-cal-rgb";
const CAL_GRAY_OPEN: &[u8] = b"color(--bd-cal-gray";
const LAB_OPEN: &[u8] = b"color(--bd-lab";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CalibratedKind {
    CalRgb,
    CalGray,
    Lab,
}

impl CalibratedKind {
    const fn marker_tag(self) -> &'static str {
        match self {
            Self::CalRgb => "cal-rgb",
            Self::CalGray => "cal-gray",
            Self::Lab => "lab",
        }
    }
}

fn contains_calibrated_call(css: &str) -> bool {
    crate::css_scan::contains_any_ascii_ci(css, &[CAL_RGB_OPEN, CAL_GRAY_OPEN, LAB_OPEN])
}

fn rewrite_owned(css: &str) -> String {
    let mut out = String::with_capacity(css.len() + 128);
    let bytes = css.as_bytes();
    let mut i = 0;
    let mut pending_markers: Vec<String> = Vec::new();

    while i < bytes.len() {
        if bytes[i] == b';' || bytes[i] == b'}' {
            for marker in pending_markers.drain(..) {
                out.push_str(&marker);
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        if let Some((kind, fn_token_len)) = match_calibrated_token(bytes, i) {
            let at_token_boundary = i == 0 || !is_ident_continue(bytes[i - 1]);
            if at_token_boundary {
                if let Some(parsed) = parse_calibrated_call(bytes, i, fn_token_len, kind) {
                    let longhand = current_property_name(&out);
                    out.push_str("color(xyz-d50 ");

                    let _ = write!(
                        out,
                        "{:.6} {:.6} {:.6}",
                        parsed.xyz_d50[0], parsed.xyz_d50[1], parsed.xyz_d50[2]
                    );
                    if let Some(alpha) = parsed.alpha_str {
                        out.push_str(" / ");
                        out.push_str(&alpha);
                    }
                    out.push(')');
                    if let Some(name) = longhand {
                        let marker = format!(
                            "; {}: {}",
                            marker_property_name(&name),
                            parsed.marker_payload
                        );
                        pending_markers.push(marker);
                    }
                    i = parsed.end;
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
    for marker in pending_markers.drain(..) {
        out.push_str(&marker);
    }
    out
}

fn match_calibrated_token(bytes: &[u8], i: usize) -> Option<(CalibratedKind, usize)> {
    crate::css_scan::match_any_ascii_ci(bytes, i, &[CAL_GRAY_OPEN, CAL_RGB_OPEN, LAB_OPEN]).map(
        |(index, length)| {
            (
                [
                    CalibratedKind::CalGray,
                    CalibratedKind::CalRgb,
                    CalibratedKind::Lab,
                ][index],
                length,
            )
        },
    )
}

struct ParsedCalibratedCall {
    xyz_d50: [f32; 3],

    alpha_str: Option<String>,

    marker_payload: String,

    end: usize,
}

fn parse_calibrated_call(
    bytes: &[u8],
    start: usize,
    fn_token_len: usize,
    kind: CalibratedKind,
) -> Option<ParsedCalibratedCall> {
    let after_open = start + fn_token_len;
    let close = find_matching_close_paren(bytes, after_open)?;
    let body = std::str::from_utf8(&bytes[after_open..close]).ok()?;
    let (head, alpha_str) = match body.split_once('/') {
        Some((h, a)) => (h.trim(), Some(a.trim().to_string())),
        None => (body.trim(), None),
    };

    let tokens: Vec<&str> = head
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();

    let (white_point, after_wp) = parse_optional_white_point(&tokens);
    let remaining = &tokens[after_wp..];

    match kind {
        CalibratedKind::CalRgb => parse_cal_rgb_args(white_point, remaining, alpha_str, close),
        CalibratedKind::CalGray => parse_cal_gray_args(white_point, remaining, alpha_str, close),
        CalibratedKind::Lab => parse_lab_args(white_point, remaining, alpha_str, close),
    }
}

fn parse_optional_white_point(tokens: &[&str]) -> ([f32; 3], usize) {
    if let Some(&first) = tokens.first() {
        if let Some(wp) = white_point_from_keyword(first) {
            return (wp, 1);
        }

        if tokens.len() >= 4
            && let (Some(x), Some(y), Some(z)) = (
                parse_component_as_f32(tokens[0]),
                parse_component_as_f32(tokens[1]),
                parse_component_as_f32(tokens[2]),
            )
        {
            return ([x, y, z], 3);
        }
    }
    (D50_WHITE_POINT, 0)
}

fn white_point_from_keyword(token: &str) -> Option<[f32; 3]> {
    match token.to_ascii_lowercase().as_str() {
        "d50" => Some(D50_WHITE_POINT),
        "d65" => Some(D65_WHITE_POINT),
        "e" => Some(ILLUMINANT_E_WHITE_POINT),
        "c" => Some(ILLUMINANT_C_WHITE_POINT),
        _ => None,
    }
}

fn parse_cal_rgb_args(
    white_point: [f32; 3],
    tokens: &[&str],
    alpha_str: Option<String>,
    close: usize,
) -> Option<ParsedCalibratedCall> {
    let (gamma, after_g) = if tokens.len() == 3 {
        (1.0_f32, 0)
    } else if tokens.len() == 4 {
        (parse_component_as_f32(tokens[0])?, 1)
    } else {
        return None;
    };
    let r = parse_component_as_f32(tokens[after_g])?;
    let g = parse_component_as_f32(tokens[after_g + 1])?;
    let b = parse_component_as_f32(tokens[after_g + 2])?;

    let lin_r = r.max(0.0).powf(gamma);
    let lin_g = g.max(0.0).powf(gamma);
    let lin_b = b.max(0.0).powf(gamma);

    let xyz_d50 = [lin_r, lin_g, lin_b];

    let marker_payload = format!(
        "{}|{:.6},{:.6},{:.6}|{:.6}|{:.6},{:.6},{:.6}",
        CalibratedKind::CalRgb.marker_tag(),
        white_point[0],
        white_point[1],
        white_point[2],
        gamma,
        r,
        g,
        b,
    );
    Some(ParsedCalibratedCall {
        xyz_d50,
        alpha_str,
        marker_payload,
        end: close + 1,
    })
}

fn parse_cal_gray_args(
    white_point: [f32; 3],
    tokens: &[&str],
    alpha_str: Option<String>,
    close: usize,
) -> Option<ParsedCalibratedCall> {
    let (gamma, after_g) = if tokens.len() == 1 {
        (1.0_f32, 0)
    } else if tokens.len() == 2 {
        (parse_component_as_f32(tokens[0])?, 1)
    } else {
        return None;
    };
    let g = parse_component_as_f32(tokens[after_g])?;
    let lin_g = g.max(0.0).powf(gamma);

    let xyz_d50 = [
        lin_g * white_point[0],
        lin_g * white_point[1],
        lin_g * white_point[2],
    ];
    let marker_payload = format!(
        "{}|{:.6},{:.6},{:.6}|{:.6}|{:.6}",
        CalibratedKind::CalGray.marker_tag(),
        white_point[0],
        white_point[1],
        white_point[2],
        gamma,
        g,
    );
    Some(ParsedCalibratedCall {
        xyz_d50,
        alpha_str,
        marker_payload,
        end: close + 1,
    })
}

fn parse_lab_args(
    white_point: [f32; 3],
    tokens: &[&str],
    alpha_str: Option<String>,
    close: usize,
) -> Option<ParsedCalibratedCall> {
    if tokens.len() != 3 {
        return None;
    }
    let l = parse_component_as_f32(tokens[0])?;
    let a = parse_component_as_f32(tokens[1])?;
    let b = parse_component_as_f32(tokens[2])?;

    let xyz_d50 = lab_to_xyz_d50(l, a, b, white_point);
    let marker_payload = format!(
        "{}|{:.6},{:.6},{:.6}|{:.6},{:.6},{:.6}",
        CalibratedKind::Lab.marker_tag(),
        white_point[0],
        white_point[1],
        white_point[2],
        l,
        a,
        b,
    );
    Some(ParsedCalibratedCall {
        xyz_d50,
        alpha_str,
        marker_payload,
        end: close + 1,
    })
}

fn lab_to_xyz_d50(l: f32, a: f32, b: f32, white: [f32; 3]) -> [f32; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let f_inv = |t: f32| -> f32 {
        const DELTA: f32 = 6.0 / 29.0;
        if t > DELTA {
            t.powi(3)
        } else {
            3.0 * DELTA * DELTA * (t - 4.0 / 29.0)
        }
    };
    [
        white[0] * f_inv(fx),
        white[1] * f_inv(fy),
        white[2] * f_inv(fz),
    ]
}

pub fn parse_marker_payload(payload: &str) -> Option<stylo_cssom_model::CalibratedColour> {
    let mut parts = payload.split('|');
    let tag = parts.next()?.trim();
    let wp_str = parts.next()?;
    let white_point = parse_xyz_csv(wp_str)?;
    match tag {
        "cal-rgb" => {
            let gamma_str = parts.next()?;
            let gamma = gamma_str.trim().parse::<f32>().ok()?;
            Some(stylo_cssom_model::CalibratedColour::CalRgb(
                stylo_cssom_model::CalRgbParams {
                    white_point,
                    black_point: None,
                    gamma: Some([gamma, gamma, gamma]),
                    matrix: None,
                },
            ))
        },
        "cal-gray" => {
            let gamma_str = parts.next()?;
            let gamma = gamma_str.trim().parse::<f32>().ok()?;
            Some(stylo_cssom_model::CalibratedColour::CalGray(
                stylo_cssom_model::CalGrayParams {
                    white_point,
                    black_point: None,
                    gamma: Some(gamma),
                },
            ))
        },
        "lab" => Some(stylo_cssom_model::CalibratedColour::Lab(
            stylo_cssom_model::LabParams {
                white_point,
                black_point: None,
                range: None,
            },
        )),
        _ => None,
    }
}

fn parse_xyz_csv(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split(',');
    let x = parts.next()?.trim().parse::<f32>().ok()?;
    let y = parts.next()?.trim().parse::<f32>().ok()?;
    let z = parts.next()?.trim().parse::<f32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}
